// async-streaming-migration.rs
// Async streaming client implementation with unified cancellation

use anyhow::Result;
use futures::stream::{Stream, StreamExt};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::interval;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Gemini,
    Local(String), // Local model name
}

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub timeout: Duration,
    pub buffer_size: usize,
    pub backpressure_threshold: usize,
    pub max_retries: u32,
    pub retry_delay: Duration,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            buffer_size: 1024,
            backpressure_threshold: 100,
            max_retries: 3,
            retry_delay: Duration::from_millis(500),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub content: String,
    pub role: Option<String>,
    pub finish_reason: Option<String>,
    pub token_count: Option<u32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct StreamMetrics {
    pub chunks_received: u64,
    pub bytes_received: u64,
    pub time_to_first_chunk_ms: Option<u64>,
    pub total_duration_ms: u64,
    pub backpressure_events: u32,
    pub retry_count: u32,
}

// ============================================================================
// Cancellation System
// ============================================================================

#[derive(Clone)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
    #[allow(clippy::type_complexity)]
    callbacks: Arc<tokio::sync::Mutex<Vec<Box<dyn FnOnce() + Send>>>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            callbacks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);

        // Execute cancellation callbacks
        tokio::spawn({
            let callbacks = Arc::clone(&self.callbacks);
            async move {
                let mut cbs = callbacks.lock().await;
                for callback in cbs.drain(..) {
                    callback();
                }
            }
        });
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    pub async fn on_cancel<F>(&self, callback: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut callbacks = self.callbacks.lock().await;
        callbacks.push(Box::new(callback));
    }

    pub fn child(&self) -> CancellationToken {
        Self {
            flag: Arc::clone(&self.flag),
            callbacks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
}

// ============================================================================
// Streaming Client Trait
// ============================================================================

#[async_trait::async_trait]
pub trait StreamingClient: Send + Sync {
    async fn stream(
        &self,
        request: StreamRequest,
        cancellation: CancellationToken,
    ) -> Result<StreamingResponse>;

    fn provider(&self) -> Provider;
}

pub struct StreamRequest {
    pub messages: Vec<Message>,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
    pub system: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub struct StreamingResponse {
    pub stream: Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>,
    pub metrics: Arc<tokio::sync::Mutex<StreamMetrics>>,
    pub cancellation: CancellationToken,
}

// ============================================================================
// Backpressure Manager
// ============================================================================

pub struct BackpressureManager {
    buffer: Vec<StreamChunk>,
    last_flush: tokio::time::Instant,
    target_interval: Duration,
    threshold: usize,
    metrics: Arc<tokio::sync::Mutex<StreamMetrics>>,
}

impl BackpressureManager {
    pub fn new(
        target_fps: f32,
        threshold: usize,
        metrics: Arc<tokio::sync::Mutex<StreamMetrics>>,
    ) -> Self {
        Self {
            buffer: Vec::with_capacity(threshold),
            last_flush: tokio::time::Instant::now(),
            target_interval: Duration::from_secs_f32(1.0 / target_fps),
            threshold,
            metrics,
        }
    }

    pub async fn push(&mut self, chunk: StreamChunk) -> Option<Vec<StreamChunk>> {
        self.buffer.push(chunk);

        if self.should_flush().await {
            Some(self.flush().await)
        } else {
            None
        }
    }

    async fn should_flush(&self) -> bool {
        self.buffer.len() >= self.threshold || self.last_flush.elapsed() >= self.target_interval
    }

    pub async fn flush(&mut self) -> Vec<StreamChunk> {
        if !self.buffer.is_empty() {
            let mut metrics = self.metrics.lock().await;
            metrics.backpressure_events += 1;
        }

        self.last_flush = tokio::time::Instant::now();
        std::mem::take(&mut self.buffer)
    }

    pub fn pending_count(&self) -> usize {
        self.buffer.len()
    }
}

// ============================================================================
// OpenAI Implementation
// ============================================================================

pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
    config: StreamConfig,
}

impl OpenAIClient {
    pub fn new(api_key: String, config: StreamConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(config.timeout)
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            config,
        }
    }

    async fn parse_sse_stream(
        &self,
        response: Response,
        cancellation: CancellationToken,
        metrics: Arc<tokio::sync::Mutex<StreamMetrics>>,
    ) -> impl Stream<Item = Result<StreamChunk>> {
        let (tx, mut rx) = mpsc::channel::<Result<StreamChunk>>(self.config.buffer_size);

        tokio::spawn(async move {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();
            let start = tokio::time::Instant::now();
            let mut first_chunk = true;

            while let Some(result) = bytes_stream.next().await {
                if cancellation.is_cancelled() {
                    break;
                }

                match result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete SSE events
                        while let Some(event_end) = buffer.find("\n\n") {
                            let event_block = buffer[..event_end].to_string();
                            buffer = buffer[event_end + 2..].to_string();

                            if let Some(data) = event_block.strip_prefix("data: ") {
                                if data.trim() == "[DONE]" {
                                    return;
                                }

                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    let chunk = StreamChunk {
                                        content: json["choices"][0]["delta"]["content"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string(),
                                        role: json["choices"][0]["delta"]["role"]
                                            .as_str()
                                            .map(|s| s.to_string()),
                                        finish_reason: json["choices"][0]["finish_reason"]
                                            .as_str()
                                            .map(|s| s.to_string()),
                                        token_count: json["usage"]["completion_tokens"]
                                            .as_u64()
                                            .map(|n| n as u32),
                                        metadata: Some(json.clone()),
                                    };

                                    // Update metrics
                                    let mut m = metrics.lock().await;
                                    m.chunks_received += 1;
                                    m.bytes_received += bytes.len() as u64;
                                    if first_chunk {
                                        m.time_to_first_chunk_ms =
                                            Some(start.elapsed().as_millis() as u64);
                                        first_chunk = false;
                                    }
                                    drop(m);

                                    if tx.send(Ok(chunk)).await.is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(anyhow::anyhow!("Stream error: {}", e))).await;
                        return;
                    }
                }
            }
        });

        // Convert channel receiver to Stream
        async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        }
    }
}

#[async_trait::async_trait]
impl StreamingClient for OpenAIClient {
    async fn stream(
        &self,
        request: StreamRequest,
        cancellation: CancellationToken,
    ) -> Result<StreamingResponse> {
        let mut retry_count = 0;
        let metrics = Arc::new(tokio::sync::Mutex::new(StreamMetrics {
            chunks_received: 0,
            bytes_received: 0,
            time_to_first_chunk_ms: None,
            total_duration_ms: 0,
            backpressure_events: 0,
            retry_count: 0,
        }));

        loop {
            let payload = serde_json::json!({
                "model": request.model,
                "messages": request.messages.iter().map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    })
                }).collect::<Vec<_>>(),
                "stream": true,
                "temperature": request.temperature,
                "max_tokens": request.max_tokens,
            });

            let response = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let stream = self
                        .parse_sse_stream(resp, cancellation.clone(), Arc::clone(&metrics))
                        .await;

                    return Ok(StreamingResponse {
                        stream: Box::pin(stream),
                        metrics,
                        cancellation,
                    });
                }
                Ok(resp) => {
                    let status = resp.status();
                    let error_body = resp.text().await.unwrap_or_default();

                    if status.is_server_error() && retry_count < self.config.max_retries {
                        retry_count += 1;
                        let mut m = metrics.lock().await;
                        m.retry_count = retry_count;
                        drop(m);

                        tokio::time::sleep(self.config.retry_delay * retry_count).await;
                        continue;
                    }

                    return Err(anyhow::anyhow!("API error {}: {}", status, error_body));
                }
                Err(_e) if retry_count < self.config.max_retries => {
                    retry_count += 1;
                    let mut m = metrics.lock().await;
                    m.retry_count = retry_count;
                    drop(m);

                    tokio::time::sleep(self.config.retry_delay * retry_count).await;
                    continue;
                }
                Err(e) => return Err(anyhow::anyhow!("Request failed: {}", e)),
            }
        }
    }

    fn provider(&self) -> Provider {
        Provider::OpenAI
    }
}

// ============================================================================
// Anthropic Implementation
// ============================================================================

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
    config: StreamConfig,
}

impl AnthropicClient {
    pub fn new(api_key: String, config: StreamConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(config.timeout)
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
            config,
        }
    }

    async fn parse_event_stream(
        &self,
        response: Response,
        cancellation: CancellationToken,
        metrics: Arc<tokio::sync::Mutex<StreamMetrics>>,
    ) -> impl Stream<Item = Result<StreamChunk>> {
        let (tx, mut rx) = mpsc::channel::<Result<StreamChunk>>(self.config.buffer_size);

        tokio::spawn(async move {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();
            let start = tokio::time::Instant::now();
            let mut first_chunk = true;
            let mut current_block = String::new();

            while let Some(result) = bytes_stream.next().await {
                if cancellation.is_cancelled() {
                    break;
                }

                match result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete SSE events
                        while let Some(event_end) = buffer.find("\n\n") {
                            let event_block = buffer[..event_end].to_string();
                            buffer = buffer[event_end + 2..].to_string();

                            // Parse event type and data
                            let mut event_type = None;
                            let mut event_data = None;

                            for line in event_block.lines() {
                                if let Some(evt) = line.strip_prefix("event: ") {
                                    event_type = Some(evt.to_string());
                                } else if let Some(data) = line.strip_prefix("data: ") {
                                    event_data = Some(data.to_string());
                                }
                            }

                            if let (Some(evt_type), Some(data)) = (event_type, event_data) {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                                    let chunk = match evt_type.as_str() {
                                        "message_start" => StreamChunk {
                                            content: String::new(),
                                            role: Some("assistant".to_string()),
                                            finish_reason: None,
                                            token_count: json["message"]["usage"]["input_tokens"]
                                                .as_u64()
                                                .map(|n| n as u32),
                                            metadata: Some(json.clone()),
                                        },
                                        "content_block_start" => {
                                            current_block.clear();
                                            StreamChunk {
                                                content: String::new(),
                                                role: None,
                                                finish_reason: None,
                                                token_count: None,
                                                metadata: Some(json.clone()),
                                            }
                                        }
                                        "content_block_delta" => {
                                            let text = json["delta"]["text"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            current_block.push_str(&text);
                                            StreamChunk {
                                                content: text,
                                                role: None,
                                                finish_reason: None,
                                                token_count: None,
                                                metadata: Some(json.clone()),
                                            }
                                        }
                                        "content_block_stop" => StreamChunk {
                                            content: String::new(),
                                            role: None,
                                            finish_reason: None,
                                            token_count: None,
                                            metadata: Some(json.clone()),
                                        },
                                        "message_delta" => StreamChunk {
                                            content: String::new(),
                                            role: None,
                                            finish_reason: json["delta"]["stop_reason"]
                                                .as_str()
                                                .map(|s| s.to_string()),
                                            token_count: json["usage"]["output_tokens"]
                                                .as_u64()
                                                .map(|n| n as u32),
                                            metadata: Some(json.clone()),
                                        },
                                        "message_stop" => StreamChunk {
                                            content: String::new(),
                                            role: None,
                                            finish_reason: Some("stop".to_string()),
                                            token_count: None,
                                            metadata: Some(json.clone()),
                                        },
                                        "error" => {
                                            let error = json["error"]["message"]
                                                .as_str()
                                                .unwrap_or("Unknown error");
                                            let _ = tx
                                                .send(Err(anyhow::anyhow!("API error: {}", error)))
                                                .await;
                                            return;
                                        }
                                        _ => continue,
                                    };

                                    // Update metrics
                                    let mut m = metrics.lock().await;
                                    m.chunks_received += 1;
                                    m.bytes_received += bytes.len() as u64;
                                    if first_chunk && !chunk.content.is_empty() {
                                        m.time_to_first_chunk_ms =
                                            Some(start.elapsed().as_millis() as u64);
                                        first_chunk = false;
                                    }
                                    drop(m);

                                    if tx.send(Ok(chunk)).await.is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(anyhow::anyhow!("Stream error: {}", e))).await;
                        return;
                    }
                }
            }
        });

        // Convert channel receiver to Stream
        async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        }
    }
}

#[async_trait::async_trait]
impl StreamingClient for AnthropicClient {
    async fn stream(
        &self,
        request: StreamRequest,
        cancellation: CancellationToken,
    ) -> Result<StreamingResponse> {
        let mut retry_count = 0;
        let metrics = Arc::new(tokio::sync::Mutex::new(StreamMetrics {
            chunks_received: 0,
            bytes_received: 0,
            time_to_first_chunk_ms: None,
            total_duration_ms: 0,
            backpressure_events: 0,
            retry_count: 0,
        }));

        loop {
            // Convert messages to Anthropic format
            let mut messages = Vec::new();
            for msg in &request.messages {
                messages.push(serde_json::json!({
                    "role": if msg.role == "system" { "assistant" } else { &msg.role },
                    "content": msg.content,
                }));
            }

            let mut payload = serde_json::json!({
                "model": request.model,
                "messages": messages,
                "stream": true,
                "max_tokens": request.max_tokens.unwrap_or(4096),
            });

            if let Some(temp) = request.temperature {
                payload["temperature"] = serde_json::json!(temp);
            }

            if let Some(system) = &request.system {
                payload["system"] = serde_json::json!(system);
            }

            let response = self
                .client
                .post(format!("{}/messages", self.base_url))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let stream = self
                        .parse_event_stream(resp, cancellation.clone(), Arc::clone(&metrics))
                        .await;

                    return Ok(StreamingResponse {
                        stream: Box::pin(stream),
                        metrics,
                        cancellation,
                    });
                }
                Ok(resp) => {
                    let status = resp.status();
                    let error_body = resp.text().await.unwrap_or_default();

                    if status.is_server_error() && retry_count < self.config.max_retries {
                        retry_count += 1;
                        let mut m = metrics.lock().await;
                        m.retry_count = retry_count;
                        drop(m);

                        tokio::time::sleep(self.config.retry_delay * retry_count).await;
                        continue;
                    }

                    return Err(anyhow::anyhow!("API error {}: {}", status, error_body));
                }
                Err(_e) if retry_count < self.config.max_retries => {
                    retry_count += 1;
                    let mut m = metrics.lock().await;
                    m.retry_count = retry_count;
                    drop(m);

                    tokio::time::sleep(self.config.retry_delay * retry_count).await;
                    continue;
                }
                Err(e) => return Err(anyhow::anyhow!("Request failed: {}", e)),
            }
        }
    }

    fn provider(&self) -> Provider {
        Provider::Anthropic
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Unified Streaming Manager
// ============================================================================

pub struct StreamingManager {
    clients: Vec<Box<dyn StreamingClient>>,
    default_provider: Provider,
    backpressure_config: (f32, usize), // (target_fps, threshold)
}

impl StreamingManager {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
            default_provider: Provider::OpenAI,
            backpressure_config: (60.0, 10),
        }
    }

    pub fn add_client(&mut self, client: Box<dyn StreamingClient>) {
        self.clients.push(client);
    }

    pub fn set_default_provider(&mut self, provider: Provider) {
        self.default_provider = provider;
    }

    pub fn set_backpressure(&mut self, fps: f32, threshold: usize) {
        self.backpressure_config = (fps, threshold);
    }

    pub async fn stream_with_backpressure(
        &self,
        provider: Provider,
        request: StreamRequest,
        cancellation: CancellationToken,
    ) -> Result<mpsc::Receiver<Vec<StreamChunk>>> {
        // Find the appropriate client
        let client = self
            .clients
            .iter()
            .find(|c| std::mem::discriminant(&c.provider()) == std::mem::discriminant(&provider))
            .ok_or_else(|| anyhow::anyhow!("Provider {:?} not configured", provider))?;

        // Start streaming
        let mut response = client.stream(request, cancellation.clone()).await?;

        // Setup backpressure handling
        let (tx, rx) = mpsc::channel::<Vec<StreamChunk>>(10);
        let (fps, threshold) = self.backpressure_config;
        let metrics = Arc::clone(&response.metrics);

        tokio::spawn(async move {
            let mut backpressure = BackpressureManager::new(fps, threshold, metrics);
            let mut ticker = interval(Duration::from_millis(16)); // ~60Hz check

            loop {
                select! {
                    Some(chunk_result) = response.stream.next() => {
                        match chunk_result {
                            Ok(chunk) => {
                                if let Some(chunks) = backpressure.push(chunk).await {
                                    if tx.send(chunks).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Stream error: {}", e);
                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        if backpressure.pending_count() > 0 {
                            let chunks = backpressure.flush().await;
                            if tx.send(chunks).await.is_err() {
                                break;
                            }
                        }
                    }
                    _ = cancellation.cancelled() => {
                        break;
                    }
                    else => break,
                }
            }

            // Flush any remaining chunks
            if backpressure.pending_count() > 0 {
                let chunks = backpressure.flush().await;
                let _ = tx.send(chunks).await;
            }
        });

        Ok(rx)
    }
}

impl Default for StreamingManager {
    fn default() -> Self {
        Self::new()
    }
}

// Helper for cancellation
impl CancellationToken {
    async fn cancelled(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

// ============================================================================
// Usage Example
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_streaming_with_cancellation() {
        // Setup
        let mut manager = StreamingManager::new();
        let config = StreamConfig::default();

        // Add clients
        manager.add_client(Box::new(OpenAIClient::new("test_key".to_string(), config.clone())));

        manager.add_client(Box::new(AnthropicClient::new("test_key".to_string(), config.clone())));

        // Create request
        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hello, how are you?".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(150),
            stream: true,
            system: None,
        };

        // Start streaming
        let cancellation = CancellationToken::new();
        let mut receiver = manager
            .stream_with_backpressure(Provider::OpenAI, request, cancellation.clone())
            .await
            .unwrap();

        // Simulate cancellation after 2 seconds
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            cancellation.cancel();
        });

        // Process chunks
        while let Some(chunks) = receiver.recv().await {
            for chunk in chunks {
                println!("Received: {}", chunk.content);
            }
        }
    }
}

#[cfg(not(test))]
#[tokio::main]
async fn main() -> Result<()> {
    // This example is primarily covered by tests; provide a minimal main to satisfy example build
    println!("Run `cargo test --example async_streaming_example` to execute tests.");
    Ok(())
}
