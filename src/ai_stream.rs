//! Streaming AI Response Handler
//!
//! Provides streaming support for AI responses with real-time output display.
//! Uses a hybrid sync/async approach: async streaming internally, sync interface externally.

use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::config::{AiConfig, EffectiveAiSettings, ProviderType};
use crate::output::Output;

/// Streaming chunk types
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// Text content chunk
    Text(String),
    /// Streaming started
    Start,
    /// Streaming completed with final text
    Done(String),
    /// Error occurred
    Error(String),
}

/// AI Mode state for interactive conversations
#[derive(Debug, Clone, PartialEq)]
pub enum AiMode {
    /// Normal shell mode
    Off,
    /// AI conversation mode - all input goes to AI
    On,
}

impl Default for AiMode {
    fn default() -> Self {
        AiMode::Off
    }
}

/// Conversation message for context tracking
#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// AI Session manager for maintaining conversation context
pub struct AiSession {
    /// Current AI mode
    pub mode: AiMode,
    /// Conversation history
    pub messages: Vec<Message>,
    /// Maximum context window size (number of messages)
    pub max_context: usize,
    /// Session start time
    pub started_at: std::time::Instant,
}

impl Default for AiSession {
    fn default() -> Self {
        Self {
            mode: AiMode::Off,
            messages: Vec::new(),
            max_context: 20, // Keep last 20 messages
            started_at: std::time::Instant::now(),
        }
    }
}

impl AiSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter AI mode
    pub fn enter(&mut self) {
        self.mode = AiMode::On;
        self.messages.clear();
        self.started_at = std::time::Instant::now();
    }

    /// Exit AI mode
    pub fn exit(&mut self) {
        self.mode = AiMode::Off;
        // Keep messages for potential resume
    }

    /// Check if in AI mode
    pub fn is_active(&self) -> bool {
        self.mode == AiMode::On
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: "user".to_string(),
            content: content.to_string(),
        });
        self.trim_context();
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: "assistant".to_string(),
            content: content.to_string(),
        });
        self.trim_context();
    }

    /// Trim context to max size
    fn trim_context(&mut self) {
        if self.messages.len() > self.max_context {
            let excess = self.messages.len() - self.max_context;
            self.messages.drain(0..excess);
        }
    }

    /// Get conversation context for API request
    pub fn get_context(&self) -> &[Message] {
        &self.messages
    }

    /// Clear conversation history
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

/// Streaming AI generator with channel-based output
pub struct StreamingAiGenerator {
    #[allow(dead_code)]
    config: AiConfig,
    effective: EffectiveAiSettings,
}

impl StreamingAiGenerator {
    pub fn new(config: &AiConfig) -> Self {
        let effective = config.get_effective_settings();
        Self {
            config: config.clone(),
            effective,
        }
    }

    /// Generate with streaming output (blocking call with real-time display)
    /// Returns the complete response text when done
    pub fn generate_streaming(
        &self,
        query: &str,
        context: &crate::ai::llm::AiContext,
        session: Option<&AiSession>,
    ) -> Result<String, crate::ai::llm::AiError> {
        use crate::ai::llm::AiError;

        if !self.effective.enabled {
            return Err(AiError::NotEnabled);
        }

        // Create channel for streaming chunks
        let (tx, rx): (Sender<StreamChunk>, Receiver<StreamChunk>) = mpsc::channel();

        // Build messages with conversation context
        let messages = self.build_messages(query, context, session);

        // Spawn streaming task based on provider
        let effective = self.effective.clone();
        let tx_clone = tx.clone();

        std::thread::spawn(move || {
            let result = match effective.provider_type {
                ProviderType::Ollama => stream_ollama(&effective, &messages, tx_clone),
                ProviderType::Claude => stream_claude(&effective, &messages, tx_clone),
                ProviderType::OpenAI
                | ProviderType::DeepSeek
                | ProviderType::Qwen
                | ProviderType::GLM
                | ProviderType::OpenRouter
                | ProviderType::Custom => {
                    stream_openai_compatible(&effective, &messages, tx_clone)
                }
                _ => {
                    // For Gemini, fall back to blocking call for now
                    stream_fallback(&effective, &messages, tx_clone)
                }
            };

            if let Err(e) = result {
                let _ = tx.send(StreamChunk::Error(e));
            }
        });

        // Process streaming output
        self.process_stream(rx)
    }

    /// Build messages array including conversation context
    fn build_messages(
        &self,
        query: &str,
        context: &crate::ai::llm::AiContext,
        session: Option<&AiSession>,
    ) -> Vec<(String, String)> {
        let mut messages = Vec::new();

        // Add system message
        messages.push(("system".to_string(), self.effective.system_prompt.clone()));

        // Add conversation history if in session
        if let Some(sess) = session {
            for msg in sess.get_context() {
                messages.push((msg.role.clone(), msg.content.clone()));
            }
        }

        // Add current user query with context
        let user_prompt = format!(
            "Working directory: {}\nShell: {}\nOS: {}\n\nUser request: {}",
            context.cwd, context.shell, context.os, query
        );
        messages.push(("user".to_string(), user_prompt));

        messages
    }

    /// Process streaming chunks and display in terminal
    fn process_stream(&self, rx: Receiver<StreamChunk>) -> Result<String, crate::ai::llm::AiError> {
        use crate::ai::llm::AiError;

        let mut full_response = String::new();
        let mut started = false;
        let timeout = Duration::from_secs(self.effective.timeout_secs);

        loop {
            match rx.recv_timeout(timeout) {
                Ok(chunk) => match chunk {
                    StreamChunk::Start => {
                        started = true;
                        // Clear any previous output artifacts
                        print!("\r\x1b[K"); // Clear line
                        io::stdout().flush().ok();
                    }
                    StreamChunk::Text(text) => {
                        if !started {
                            started = true;
                            print!("\r\x1b[K");
                        }
                        // Print text chunk in real-time
                        print!("{}", text);
                        io::stdout().flush().ok();
                        full_response.push_str(&text);
                    }
                    StreamChunk::Done(final_text) => {
                        if !final_text.is_empty() && full_response.is_empty() {
                            full_response = final_text;
                        }
                        println!(); // Newline at end
                        return Ok(full_response);
                    }
                    StreamChunk::Error(e) => {
                        println!(); // Newline before error
                        return Err(AiError::ApiError(e));
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(AiError::Timeout);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Channel closed, return what we have
                    if !full_response.is_empty() {
                        println!();
                        return Ok(full_response);
                    }
                    return Err(AiError::NetworkError("Connection closed".to_string()));
                }
            }
        }
    }
}

/// Stream from Ollama API (newline-delimited JSON)
fn stream_ollama(
    effective: &EffectiveAiSettings,
    messages: &[(String, String)],
    tx: Sender<StreamChunk>,
) -> Result<(), String> {
    use std::io::BufRead;

    let model = effective
        .model
        .clone()
        .unwrap_or_else(|| "qwen2.5:7b".to_string());

    let endpoint = effective
        .endpoint
        .clone()
        .unwrap_or_else(|| "http://localhost:11434/api/chat".to_string());

    // Build Ollama request
    let ollama_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|(role, content)| {
            serde_json::json!({
                "role": role,
                "content": content
            })
        })
        .collect();

    let request_body = serde_json::json!({
        "model": model,
        "messages": ollama_messages,
        "stream": true
    });

    // Create blocking HTTP client with streaming
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(effective.timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(&endpoint)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Status {}: {}", status, body));
    }

    // Send start signal
    let _ = tx.send(StreamChunk::Start);

    // Process streaming response line by line
    let reader = std::io::BufReader::new(response);
    let mut full_response = String::new();

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.is_empty() {
            continue;
        }

        // Parse Ollama streaming JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            // Check for content in message
            if let Some(message) = json.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                    if !content.is_empty() {
                        full_response.push_str(content);
                        let _ = tx.send(StreamChunk::Text(content.to_string()));
                    }
                }
            }

            // Check if done
            if json.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
                let _ = tx.send(StreamChunk::Done(full_response));
                return Ok(());
            }
        }
    }

    // If we get here, send what we have
    let _ = tx.send(StreamChunk::Done(full_response));
    Ok(())
}

/// Stream from Claude API (SSE format with different event structure)
fn stream_claude(
    effective: &EffectiveAiSettings,
    messages: &[(String, String)],
    tx: Sender<StreamChunk>,
) -> Result<(), String> {
    use std::io::BufRead;

    let model = effective
        .model
        .clone()
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let endpoint = effective
        .endpoint
        .clone()
        .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string());

    // Resolve API key
    let api_key = effective
        .api_key
        .as_ref()
        .and_then(|key| {
            if key.starts_with('$') {
                std::env::var(&key[1..]).ok()
            } else {
                Some(key.clone())
            }
        })
        .ok_or_else(|| "API key not configured".to_string())?;

    // Build Claude messages (system is separate)
    let system_prompt = messages
        .iter()
        .find(|(role, _)| role == "system")
        .map(|(_, content)| content.clone());

    let claude_messages: Vec<serde_json::Value> = messages
        .iter()
        .filter(|(role, _)| role != "system")
        .map(|(role, content)| {
            serde_json::json!({
                "role": role,
                "content": content
            })
        })
        .collect();

    let mut request_body = serde_json::json!({
        "model": model,
        "max_tokens": effective.max_tokens,
        "messages": claude_messages,
        "stream": true
    });

    if let Some(system) = system_prompt {
        request_body["system"] = serde_json::Value::String(system);
    }

    // Create HTTP client
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(effective.timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(&endpoint)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Status {}: {}", status, body));
    }

    // Send start signal
    let _ = tx.send(StreamChunk::Start);

    // Process SSE stream
    let reader = std::io::BufReader::new(response);
    let mut full_response = String::new();

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;

        // Claude SSE format: "event: <type>\ndata: {...}"
        if !line.starts_with("data: ") {
            continue;
        }

        let data = &line[6..];

        // Parse JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            // Check event type
            let event_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match event_type {
                "content_block_delta" => {
                    // Extract text from delta
                    if let Some(delta) = json.get("delta") {
                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                            if !text.is_empty() {
                                full_response.push_str(text);
                                let _ = tx.send(StreamChunk::Text(text.to_string()));
                            }
                        }
                    }
                }
                "message_stop" => {
                    let _ = tx.send(StreamChunk::Done(full_response));
                    return Ok(());
                }
                "error" => {
                    let error_msg = json
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    return Err(error_msg.to_string());
                }
                _ => {}
            }
        }
    }

    // If we get here without message_stop, send what we have
    let _ = tx.send(StreamChunk::Done(full_response));
    Ok(())
}

/// Stream from OpenAI-compatible APIs (SSE format)
fn stream_openai_compatible(
    effective: &EffectiveAiSettings,
    messages: &[(String, String)],
    tx: Sender<StreamChunk>,
) -> Result<(), String> {
    use std::io::BufRead;

    let model = effective
        .model
        .clone()
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    // Get endpoint based on provider type
    let endpoint = effective.endpoint.clone().unwrap_or_else(|| {
        match effective.provider_type {
            ProviderType::OpenAI => "https://api.openai.com/v1/chat/completions".to_string(),
            ProviderType::DeepSeek => "https://api.deepseek.com/v1/chat/completions".to_string(),
            ProviderType::Qwen => {
                "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".to_string()
            }
            ProviderType::GLM => {
                "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
            }
            ProviderType::OpenRouter => {
                "https://openrouter.ai/api/v1/chat/completions".to_string()
            }
            _ => "https://api.openai.com/v1/chat/completions".to_string(),
        }
    });

    // Resolve API key
    let api_key = effective
        .api_key
        .as_ref()
        .and_then(|key| {
            if key.starts_with('$') {
                std::env::var(&key[1..]).ok()
            } else {
                Some(key.clone())
            }
        })
        .ok_or_else(|| "API key not configured".to_string())?;

    // Build OpenAI-compatible request
    let openai_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|(role, content)| {
            serde_json::json!({
                "role": role,
                "content": content
            })
        })
        .collect();

    let request_body = serde_json::json!({
        "model": model,
        "messages": openai_messages,
        "stream": true,
        "max_tokens": effective.max_tokens,
        "temperature": effective.temperature
    });

    // Create HTTP client
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(effective.timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;

    // Build request with appropriate headers
    let mut request = client
        .post(&endpoint)
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key));

    // Add OpenRouter-specific headers
    if effective.provider_type == ProviderType::OpenRouter {
        request = request
            .header("HTTP-Referer", "https://github.com/skingford/smart-command")
            .header("X-Title", "Smart Command");
    }

    let response = request.json(&request_body).send().map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Status {}: {}", status, body));
    }

    // Send start signal
    let _ = tx.send(StreamChunk::Start);

    // Process SSE stream
    let reader = std::io::BufReader::new(response);
    let mut full_response = String::new();

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;

        // SSE format: "data: {...}" or "data: [DONE]"
        if !line.starts_with("data: ") {
            continue;
        }

        let data = &line[6..]; // Strip "data: " prefix

        if data == "[DONE]" {
            let _ = tx.send(StreamChunk::Done(full_response));
            return Ok(());
        }

        // Parse JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            // Extract content from choices[0].delta.content
            if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
                if let Some(first_choice) = choices.first() {
                    if let Some(delta) = first_choice.get("delta") {
                        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                            if !content.is_empty() {
                                full_response.push_str(content);
                                let _ = tx.send(StreamChunk::Text(content.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    // If we get here without [DONE], send what we have
    let _ = tx.send(StreamChunk::Done(full_response));
    Ok(())
}

/// Fallback for non-streaming providers (uses blocking API)
fn stream_fallback(
    effective: &EffectiveAiSettings,
    messages: &[(String, String)],
    tx: Sender<StreamChunk>,
) -> Result<(), String> {
    // Show spinner while waiting
    let _ = tx.send(StreamChunk::Start);

    // Use the existing blocking implementation
    let config = crate::config::AiConfig {
        enabled: effective.enabled,
        active: "fallback".to_string(),
        providers: {
            let mut providers = std::collections::HashMap::new();
            providers.insert(
                "fallback".to_string(),
                crate::config::ProviderConfig {
                    provider_type: effective.provider_type.clone(),
                    api_key: effective.api_key.clone(),
                    endpoint: effective.endpoint.clone(),
                    model: effective.model.clone(),
                    max_tokens: Some(effective.max_tokens),
                    temperature: Some(effective.temperature),
                    timeout_secs: Some(effective.timeout_secs),
                },
            );
            providers
        },
        global: crate::config::GlobalAiSettings {
            system_prompt: effective.system_prompt.clone(),
            max_tokens: effective.max_tokens,
            temperature: effective.temperature,
            timeout_secs: effective.timeout_secs,
            warn_dangerous: true,
        },
        ..Default::default()
    };

    let generator = crate::ai::llm::AiCommandGenerator::new(&config);

    // Extract query from messages (last user message)
    let query = messages
        .iter()
        .rev()
        .find(|(role, _)| role == "user")
        .map(|(_, content)| content.as_str())
        .unwrap_or("");

    let context = crate::ai::llm::AiContext::default();

    match generator.generate(query, &context) {
        Ok(response) => {
            let _ = tx.send(StreamChunk::Text(response.clone()));
            let _ = tx.send(StreamChunk::Done(response));
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Display streaming status indicator
#[allow(dead_code)]
pub fn show_streaming_indicator(provider: &str) {
    Output::dim(&format!("  {} streaming...", provider));
}

/// Parse AI mode command
pub fn parse_ai_mode_command(input: &str) -> Option<AiModeCommand> {
    let trimmed = input.trim().to_lowercase();

    match trimmed.as_str() {
        "ai on" | "ai start" | "ai enter" | "ai mode" => Some(AiModeCommand::Enter),
        "ai off" | "ai exit" | "ai stop" | "ai quit" | "/exit" | "/quit" | "/q" => {
            Some(AiModeCommand::Exit)
        }
        "ai clear" | "/clear" | "/c" => Some(AiModeCommand::Clear),
        "ai help" | "/help" | "/h" | "/?" => Some(AiModeCommand::Help),
        _ => None,
    }
}

/// AI mode commands
#[derive(Debug, Clone, PartialEq)]
pub enum AiModeCommand {
    /// Enter AI mode
    Enter,
    /// Exit AI mode
    Exit,
    /// Clear conversation history
    Clear,
    /// Show help
    Help,
}

/// Display AI mode help
pub fn show_ai_mode_help() {
    println!();
    Output::info("AI Mode Commands");
    println!();
    println!("  {}      - Exit AI mode and return to shell", nu_ansi_term::Color::Cyan.paint("/exit"));
    println!("  {}     - Clear conversation history", nu_ansi_term::Color::Cyan.paint("/clear"));
    println!("  {}      - Show this help", nu_ansi_term::Color::Cyan.paint("/help"));
    println!();
    Output::dim("In AI mode, all input is sent to the AI for command generation.");
    Output::dim("The AI remembers your conversation context for follow-up questions.");
    println!();
}

/// Display AI mode welcome message
pub fn show_ai_mode_welcome(provider: &str, model: Option<&str>) {
    println!();
    Output::success("Entered AI conversation mode");
    Output::dim(&format!(
        "  Provider: {} | Model: {}",
        provider,
        model.unwrap_or("default")
    ));
    Output::dim("  Type your request in natural language");
    Output::dim("  Commands: /exit (quit) | /clear (reset) | /help");
    println!();
}

/// Display AI mode exit message
pub fn show_ai_mode_exit() {
    Output::dim("Exited AI mode. Back to normal shell.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_session() {
        let mut session = AiSession::new();
        assert!(!session.is_active());

        session.enter();
        assert!(session.is_active());

        session.add_user_message("test");
        assert_eq!(session.messages.len(), 1);

        session.add_assistant_message("response");
        assert_eq!(session.messages.len(), 2);

        session.exit();
        assert!(!session.is_active());
    }

    #[test]
    fn test_parse_ai_mode_command() {
        assert_eq!(parse_ai_mode_command("ai on"), Some(AiModeCommand::Enter));
        assert_eq!(parse_ai_mode_command("ai off"), Some(AiModeCommand::Exit));
        assert_eq!(parse_ai_mode_command("/exit"), Some(AiModeCommand::Exit));
        assert_eq!(parse_ai_mode_command("/clear"), Some(AiModeCommand::Clear));
        assert_eq!(parse_ai_mode_command("random"), None);
    }
}
