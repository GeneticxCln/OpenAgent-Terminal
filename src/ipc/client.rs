// IPC Client - Unix Socket Connection to Python Backend

use super::error::IpcError;
use super::message::{Notification, Request, Response};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

type RequestId = u64;
type ResponseSender = tokio::sync::oneshot::Sender<Result<Response, IpcError>>;

/// Request ID space boundaries for collision prevention
/// Interactive flow uses 0-9999, SessionManager uses 10000+
const INTERACTIVE_ID_MIN: u64 = 0;
const INTERACTIVE_ID_MAX: u64 = 9999;
#[allow(dead_code)] // Used by SessionManager in session.rs module
const SESSION_MANAGER_ID_MIN: u64 = 10000;

/// Connection state for the IPC client
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected to backend
    Disconnected,
    /// Currently attempting to connect
    Connecting,
    /// Successfully connected and operational
    Connected,
    /// Connection lost, attempting to reconnect
    Reconnecting { attempt: u32 },
    /// Reconnection failed after max attempts
    Failed,
}

/// IPC client for communication with Python backend
pub struct IpcClient {
    write_sender: Option<mpsc::UnboundedSender<String>>,
    request_counter: u64,
    pending_requests: Arc<Mutex<HashMap<RequestId, ResponseSender>>>,
    notification_sender: Option<mpsc::UnboundedSender<Notification>>,
    notification_receiver: Option<mpsc::UnboundedReceiver<Notification>>,
    connection_state: ConnectionState,
    socket_path: Option<String>,
}

impl IpcClient {
    /// Create a new IPC client (not connected)
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            write_sender: None,
            request_counter: 0,
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            notification_sender: Some(tx),
            notification_receiver: Some(rx),
            connection_state: ConnectionState::Disconnected,
            socket_path: None,
        }
    }

    /// Connect to the Python backend via Unix socket with retry logic
    pub async fn connect(&mut self, socket_path: &str) -> Result<(), IpcError> {
        self.socket_path = Some(socket_path.to_string());
        self.connect_with_retry(socket_path, 3).await
    }

    /// Connect with specified number of retry attempts
    pub async fn connect_with_retry(&mut self, socket_path: &str, max_attempts: u32) -> Result<(), IpcError> {
        info!("🔌 Connecting to Python backend at {}", socket_path);
        self.connection_state = ConnectionState::Connecting;
        
        let mut last_error = None;
        
        for attempt in 0..max_attempts {
            if attempt > 0 {
                self.connection_state = ConnectionState::Reconnecting { attempt };
                let delay = std::time::Duration::from_millis(200 * (2_u64.pow(attempt - 1)));
                info!("🔄 Reconnection attempt {} after {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
            }
            
            match UnixStream::connect(socket_path).await {
                Ok(stream) => {
                    info!("✅ Connected to Unix socket");
                    
                    // Start the message handling task
                    self.start_message_handler(stream).await?;
                    self.connection_state = ConnectionState::Connected;
                    
                    return Ok(());
                }
                Err(e) => {
                    warn!("⚠️  Connection attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }
        
        self.connection_state = ConnectionState::Failed;
        Err(IpcError::ConnectionError(
            format!("Failed to connect after {} attempts. Last error: {}", 
                    max_attempts, 
                    last_error.unwrap())
        ))
    }

    /// Start the background task that handles incoming messages
    async fn start_message_handler(&mut self, stream: UnixStream) -> Result<(), IpcError> {
        let (read_half, write_half) = stream.into_split();
        let reader = BufReader::new(read_half);
        
        // Create channel for writing messages
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<String>();
        
        // Store the write sender
        self.write_sender = Some(write_tx);
        
        // Spawn task to handle writes
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut writer = write_half;
            
            while let Some(message) = write_rx.recv().await {
                if let Err(e) = writer.write_all(message.as_bytes()).await {
                    error!("❌ Write failed: {} - Connection lost", e);
                    break;
                }
                if let Err(e) = writer.write_all(b"\n").await {
                    error!("❌ Write newline failed: {} - Connection lost", e);
                    break;
                }
                if let Err(e) = writer.flush().await {
                    error!("❌ Flush failed: {} - Connection lost", e);
                    break;
                }
            }
            warn!("🔌 Write handler task ended - connection lost");
        });
        
        let pending_requests = Arc::clone(&self.pending_requests);
        let notification_sender = self.notification_sender.take()
            .ok_or_else(|| IpcError::InternalError("Notification sender not available".to_string()))?;
        
        // Spawn background task to handle incoming messages
        tokio::spawn(async move {
            let mut lines = reader.lines();
            
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        debug!("📨 Received: {}", line);
                        
                        if let Err(e) = Self::handle_incoming_message(
                            &line, 
                            &pending_requests, 
                            &notification_sender
                        ).await {
                            warn!("Failed to handle message: {}", e);
                        }
                    }
                    Ok(None) => {
                        warn!("🔌 Connection closed by backend (EOF received)");
                        break;
                    }
                    Err(e) => {
                        error!("❌ Read error: {} - Connection lost", e);
                        break;
                    }
                }
            }
            
            warn!("🔌 Message handler task ended - connection lost");
            // Note: In a production system, you might want to notify the main client
            // about the disconnection via a channel so it can attempt reconnection
        });
        
        Ok(())
    }
    
    /// Handle an incoming message (response or notification)
    async fn handle_incoming_message(
        line: &str,
        pending_requests: &Arc<Mutex<HashMap<RequestId, ResponseSender>>>,
        notification_sender: &mpsc::UnboundedSender<Notification>,
    ) -> Result<(), IpcError> {
        // First, check for unknown fields using tolerant parsing
        Self::check_for_unknown_fields(line);
        
        // Try to parse as notification first (no 'id' field)
        match serde_json::from_str::<Notification>(line) {
            Ok(notification) => {
                debug!("📬 Received notification: {}", notification.method);
                if let Err(_) = notification_sender.send(notification) {
                    warn!("Failed to send notification - receiver dropped");
                }
                return Ok(());
            }
            Err(e) => {
                debug!("Not a notification: {}", e);
            }
        }
        
        // Try to parse as response
        match serde_json::from_str::<Response>(line) {
            Ok(response) => {
                if let super::message::RequestId::Number(id) = response.id {
                    debug!("📬 Received response for request {}", id);
                    let mut pending = pending_requests.lock().unwrap();
                    if let Some(sender) = pending.remove(&id) {
                        let _ = sender.send(Ok(response));
                    } else {
                        warn!("Received response for unknown request ID: {}", id);
                    }
                }
                return Ok(());
            }
            Err(e) => {
                return Err(IpcError::ParseError(format!("Failed to parse message: {} (error: {})", line, e)));
            }
        }
    }
    
    /// Check for unknown fields in incoming messages (for protocol drift detection)
    fn check_for_unknown_fields(line: &str) {
        use super::message::TolerantMessage;
        if let Ok(tolerant) = serde_json::from_str::<TolerantMessage>(line) {
            tolerant.log_unknown_fields();
        }
    }

    /// Send initialize request and wait for response
    pub async fn initialize(&mut self) -> Result<Response, IpcError> {
        info!("🚀 Sending initialize request");
        
        let request = Request::initialize(self.next_request_id());
        
        self.send_request(request).await
    }

    /// Send a request and wait for response
    pub async fn send_request(&mut self, request: Request) -> Result<Response, IpcError> {
        let write_sender = self.write_sender.as_ref()
            .ok_or_else(|| IpcError::NotConnected)?;
            
        let request_id = match &request.id {
            super::message::RequestId::Number(id) => *id,
            super::message::RequestId::String(_) => {
                return Err(IpcError::InternalError("String IDs not supported yet".to_string()));
            }
        };
        
        // Create channel for response
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        // Register the pending request
        {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.insert(request_id, tx);
        }
        
        // Serialize and send the request
        let message = serde_json::to_string(&request)
            .map_err(|e| IpcError::SerializationError(e.to_string()))?;
            
        debug!("📤 Sending: {}", message);
        
        write_sender.send(message)
            .map_err(|_| IpcError::ConnectionError("Write channel closed".to_string()))?;
        
        // Wait for response with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30), 
            rx
        ).await;
        
        // Clean up pending request on timeout to prevent memory leak
        match result {
            Ok(response_result) => {
                // Response received before timeout
                response_result
                    .map_err(|_| IpcError::InternalError("Response channel closed".to_string()))?
            }
            Err(_) => {
                // Timeout occurred - clean up pending request to prevent memory leak!
                let mut pending = self.pending_requests.lock().unwrap();
                pending.remove(&request_id);
                warn!("Request {} timed out after 30s, cleaned up pending entry", request_id);
                Err(IpcError::Timeout)
            }
        }
    }

    /// Check for incoming notifications (non-blocking)
    #[allow(dead_code)] // Kept for backward compatibility
    pub async fn poll_notifications(&mut self) -> Result<Vec<Notification>, IpcError> {
        let receiver = self.notification_receiver.as_mut()
            .ok_or_else(|| IpcError::InternalError("Notification receiver not available".to_string()))?;
            
        let mut notifications = Vec::new();
        
        while let Ok(notification) = receiver.try_recv() {
            notifications.push(notification);
        }
        
        Ok(notifications)
    }
    
    /// Wait for the next notification (blocking async)
    pub async fn next_notification(&mut self) -> Result<Notification, IpcError> {
        let receiver = self.notification_receiver.as_mut()
            .ok_or_else(|| IpcError::InternalError("Notification receiver not available".to_string()))?;
            
        receiver.recv().await
            .ok_or_else(|| IpcError::InternalError("Notification channel closed".to_string()))
    }
    
    /// Send a notification to the backend (fire-and-forget, no response expected)
    pub async fn send_notification(&mut self, notification: Notification) -> Result<(), IpcError> {
        let write_sender = self.write_sender.as_ref()
            .ok_or_else(|| IpcError::NotConnected)?;
        
        // Serialize and send the notification
        let message = serde_json::to_string(&notification)
            .map_err(|e| IpcError::SerializationError(e.to_string()))?;
        
        debug!("📤 Sending notification: {}", message);
        
        write_sender.send(message)
            .map_err(|_| IpcError::ConnectionError("Write channel closed".to_string()))?;
        
        Ok(())
    }

    /// Attempt to reconnect to the backend
    #[allow(dead_code)] // For future use in connection health monitoring
    pub async fn reconnect(&mut self) -> Result<(), IpcError> {
        if let Some(socket_path) = &self.socket_path.clone() {
            info!("🔄 Attempting to reconnect to backend...");
            self.connect_with_retry(socket_path, 5).await
        } else {
            Err(IpcError::InternalError("No socket path stored for reconnection".to_string()))
        }
    }
    
    /// Disconnect and clean up
    pub async fn disconnect(&mut self) -> Result<(), IpcError> {
        info!("🔌 Disconnecting from backend");
        
        // Drop write sender to close channel
        self.write_sender = None;
        self.connection_state = ConnectionState::Disconnected;
        
        // Clear pending requests
        self.pending_requests.lock().unwrap().clear();
        
        info!("✅ Disconnected");
        Ok(())
    }
    
    /// Get the next request ID (for interactive flow: 0-9999)
    pub fn next_request_id(&mut self) -> u64 {
        self.request_counter += 1;
        // Wrap around to prevent collision with SessionManager IDs
        if self.request_counter > INTERACTIVE_ID_MAX {
            warn!("⚠️  Interactive request ID wrapped around (exceeded {})", INTERACTIVE_ID_MAX);
            self.request_counter = INTERACTIVE_ID_MIN + 1;
        }
        self.request_counter
    }
    
    /// Check if connected
    #[allow(dead_code)] // Public API for connection state checks
    pub fn is_connected(&self) -> bool {
        self.connection_state == ConnectionState::Connected
    }
    
    /// Get the current connection state
    #[allow(dead_code)] // Public API for detailed connection state
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_state
    }
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ipc_client_creation() {
        let _client = IpcClient::new();
        // TODO: Add more tests as implementation progresses
    }
}
