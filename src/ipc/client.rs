// IPC Client - Unix Socket Connection to Python Backend

use super::error::IpcError;
use super::message::{Notification, Request, Response};
use anyhow::Result;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

type RequestId = u64;
type ResponseSender = tokio::sync::oneshot::Sender<Result<Response, IpcError>>;

/// IPC client for communication with Python backend
pub struct IpcClient {
    write_sender: Option<mpsc::UnboundedSender<String>>,
    request_counter: u64,
    pending_requests: Arc<Mutex<HashMap<RequestId, ResponseSender>>>,
    notification_sender: Option<mpsc::UnboundedSender<Notification>>,
    notification_receiver: Option<mpsc::UnboundedReceiver<Notification>>,
    connected: bool,
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
            connected: false,
        }
    }

    /// Connect to the Python backend via Unix socket with retry logic
    pub async fn connect(&mut self, socket_path: &str) -> Result<(), IpcError> {
        self.connect_with_retry(socket_path, 3).await
    }

    /// Connect with specified number of retry attempts
    pub async fn connect_with_retry(&mut self, socket_path: &str, max_attempts: u32) -> Result<(), IpcError> {
        info!("🔌 Connecting to Python backend at {}", socket_path);
        
        let mut last_error = None;
        
        for attempt in 0..max_attempts {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(200 * (2_u64.pow(attempt - 1)));
                info!("Retry attempt {} after {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
            }
            
            match UnixStream::connect(socket_path).await {
                Ok(stream) => {
                    info!("✅ Connected to Unix socket");
                    
                    // Start the message handling task
                    self.start_message_handler(stream).await?;
                    self.connected = true;
                    
                    return Ok(());
                }
                Err(e) => {
                    warn!("Connection attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }
        
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
                    warn!("Write failed: {}", e);
                    break;
                }
                if let Err(e) = writer.write_all(b"\n").await {
                    warn!("Write newline failed: {}", e);
                    break;
                }
                if let Err(e) = writer.flush().await {
                    warn!("Flush failed: {}", e);
                    break;
                }
            }
            info!("Write handler task ended");
        });
        
        let pending_requests = Arc::clone(&self.pending_requests);
        let notification_sender = self.notification_sender.take()
            .ok_or_else(|| IpcError::InternalError("Notification sender not available".to_string()))?;
        
        // Spawn background task to handle incoming messages
        tokio::spawn(async move {
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("📨 Received: {}", line);
                
                if let Err(e) = Self::handle_incoming_message(
                    &line, 
                    &pending_requests, 
                    &notification_sender
                ).await {
                    warn!("Failed to handle message: {}", e);
                }
            }
            
            info!("Message handler task ended");
        });
        
        Ok(())
    }
    
    /// Handle an incoming message (response or notification)
    async fn handle_incoming_message(
        line: &str,
        pending_requests: &Arc<Mutex<HashMap<RequestId, ResponseSender>>>,
        notification_sender: &mpsc::UnboundedSender<Notification>,
    ) -> Result<(), IpcError> {
        // Try to parse as notification first (no 'id' field)
        if let Ok(notification) = serde_json::from_str::<Notification>(line) {
            debug!("📬 Received notification: {}", notification.method);
            if let Err(_) = notification_sender.send(notification) {
                warn!("Failed to send notification - receiver dropped");
            }
            return Ok(());
        }
        
        // Try to parse as response
        if let Ok(response) = serde_json::from_str::<Response>(line) {
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
        
        Err(IpcError::ParseError(format!("Failed to parse message: {}", line)))
    }

    /// Send initialize request and wait for response
    pub async fn initialize(&mut self) -> Result<Response, IpcError> {
        info!("🚀 Sending initialize request");
        
        // TODO: Get actual terminal size
        let request = Request::initialize(self.next_request_id(), 80, 24);
        
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
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(30), 
            rx
        ).await
        .map_err(|_| IpcError::Timeout)?
        .map_err(|_| IpcError::InternalError("Response channel closed".to_string()))??;
        
        Ok(response)
    }

    /// Check for incoming notifications (non-blocking)
    pub async fn poll_notifications(&mut self) -> Result<Vec<Notification>, IpcError> {
        let receiver = self.notification_receiver.as_mut()
            .ok_or_else(|| IpcError::InternalError("Notification receiver not available".to_string()))?;
            
        let mut notifications = Vec::new();
        
        while let Ok(notification) = receiver.try_recv() {
            notifications.push(notification);
        }
        
        Ok(notifications)
    }

    /// Disconnect and clean up
    pub async fn disconnect(&mut self) -> Result<(), IpcError> {
        info!("🔌 Disconnecting from backend");
        
        // Drop write sender to close channel
        self.write_sender = None;
        self.connected = false;
        
        // Clear pending requests
        self.pending_requests.lock().unwrap().clear();
        
        info!("✅ Disconnected");
        Ok(())
    }
    
    /// Get the next request ID
    pub fn next_request_id(&mut self) -> u64 {
        self.request_counter += 1;
        self.request_counter
    }
    
    /// Check if connected
    #[allow(dead_code)] // May be useful for future connection status checks
    pub fn is_connected(&self) -> bool {
        self.connected
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
