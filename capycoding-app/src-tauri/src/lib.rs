use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::types::{
    ClaudeMetricsRequest, ClaudeMetricsSnapshot, ClaudeQuestionRequest, ClaudeQuestionResponse,
    ClaudeUsage, ClaudeVoiceRequest, ClaudeVoiceResponse, LivekitTokenRequest,
    LivekitTokenResponse, PushClaudeMetricsRequest,
};

const PYTHON_METRICS_SCRIPT: &str = include_str!("python/collect_metrics.py");

#[taurpc::procedures(export_to = "../src/types.ts")]
trait Api {
    async fn collect_claude_metrics(
        request: ClaudeMetricsRequest,
    ) -> Result<ClaudeMetricsSnapshot, String>;

    async fn push_claude_metrics(
        request: PushClaudeMetricsRequest,
    ) -> Result<ClaudeMetricsSnapshot, String>;

    async fn ask_claude(request: ClaudeQuestionRequest) -> Result<ClaudeQuestionResponse, String>;

    async fn ask_claude_voice(request: ClaudeVoiceRequest) -> Result<ClaudeVoiceResponse, String>;

    async fn generate_livekit_token(
        request: LivekitTokenRequest,
    ) -> Result<LivekitTokenResponse, String>;
}

#[derive(Clone)]
struct ApiImpl {
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct PythonMetrics {
    timestamp: DateTime<Utc>,
    window_hours: f64,
    burn_rate_per_hour: f64,
    total_cost_usd: f64,
    input_tokens: i64,
    output_tokens: i64,
    cache_creation_tokens: i64,
    cache_read_tokens: i64,
    total_tokens: i64,
    session_count: i32,
    active_session_id: Option<String>,
    last_activity: DateTime<Utc>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    model: String,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(serde::Serialize)]
struct LivekitVideoGrant {
    #[serde(rename = "roomJoin")]
    room_join: bool,
    room: String,
    #[serde(rename = "canPublish", skip_serializing_if = "Option::is_none")]
    can_publish: Option<bool>,
    #[serde(rename = "canPublishData", skip_serializing_if = "Option::is_none")]
    can_publish_data: Option<bool>,
    #[serde(rename = "canSubscribe", skip_serializing_if = "Option::is_none")]
    can_subscribe: Option<bool>,
}

#[derive(serde::Serialize)]
struct LivekitClaims {
    iss: String,
    sub: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<String>,
    exp: i64,
    nbf: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
    video: LivekitVideoGrant,
}

#[taurpc::resolvers]
impl Api for ApiImpl {
    async fn collect_claude_metrics(
        self,
        request: ClaudeMetricsRequest,
    ) -> Result<ClaudeMetricsSnapshot, String> {
        let python_bin = request.python_path.unwrap_or_else(|| "python3".to_string());
        let mut command = Command::new(python_bin);
        command.arg("-");
        command.env(
            "CLAUDE_METRICS_CONFIG",
            serde_json::to_string(&json!({
                "data_dir": request.data_dir,
                "hours_back": request.hours_back,
            }))
            .map_err(|err| err.to_string())?,
        );
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|err| format!("failed to spawn python: {err}"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open python stdin".to_string())?;
        stdin
            .write_all(PYTHON_METRICS_SCRIPT.as_bytes())
            .await
            .map_err(|err| format!("failed to write python script: {err}"))?;
        drop(stdin);

        let output = child
            .wait_with_output()
            .await
            .map_err(|err| format!("failed to run python script: {err}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("python metrics collector failed: {stderr}"));
        }

        let parsed: PythonMetrics = serde_json::from_slice(&output.stdout)
            .map_err(|err| format!("failed to parse metrics: {err}"))?;

        Ok(ClaudeMetricsSnapshot {
            timestamp: parsed.timestamp.to_rfc3339(),
            window_hours: parsed.window_hours,
            burn_rate_per_hour: parsed.burn_rate_per_hour,
            total_cost_usd: parsed.total_cost_usd,
            input_tokens: parsed.input_tokens,
            output_tokens: parsed.output_tokens,
            cache_creation_tokens: parsed.cache_creation_tokens,
            cache_read_tokens: parsed.cache_read_tokens,
            total_tokens: parsed.total_tokens,
            session_count: parsed.session_count,
            active_session_id: parsed.active_session_id,
            last_activity: parsed.last_activity.to_rfc3339(),
            source: parsed.source,
        })
    }

    async fn push_claude_metrics(
        self,
        request: PushClaudeMetricsRequest,
    ) -> Result<ClaudeMetricsSnapshot, String> {
        let mut url = request.server_url.trim_end_matches('/').to_string();
        url.push_str("/metrics/claude");

        let mut builder = self.client.post(url).json(&request.metrics);
        if let Some(token) = &request.auth_token {
            if !token.is_empty() {
                builder = builder.bearer_auth(token);
            }
        }

        let response = builder
            .send()
            .await
            .map_err(|err| format!("failed to push metrics: {err}"))?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err("server rejected metrics: unauthorized".to_string());
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("server rejected metrics ({status}): {body}"));
        }

        response
            .json::<ClaudeMetricsSnapshot>()
            .await
            .map_err(|err| format!("failed to decode server response: {err}"))
    }

    async fn ask_claude(
        self,
        request: ClaudeQuestionRequest,
    ) -> Result<ClaudeQuestionResponse, String> {
        if request.api_key.trim().is_empty() {
            return Err("Claude API key is required".to_string());
        }

        let mut content = Vec::new();
        if let Some(ctx) = request.code_context.as_ref() {
            if !ctx.trim().is_empty() {
                content.push(json!({"type": "text", "text": format!("Context:\n{}", ctx)}));
            }
        }
        content.push(json!({"type": "text", "text": request.question}));

        let mut body = json!({
            "model": request
                .model
                .clone()
                .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string()),
            "max_output_tokens": request.max_output_tokens.unwrap_or(800),
            "temperature": request.temperature.unwrap_or(0.2),
            "messages": [
                {
                    "role": "user",
                    "content": content,
                }
            ]
        });

        if let Some(system) = request.system_prompt.as_ref() {
            if let Some(map) = body.as_object_mut() {
                map.insert(
                    "system".to_string(),
                    serde_json::Value::String(system.clone()),
                );
            }
        }

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", request.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|err| format!("failed to call Claude: {err}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Claude API error ({status}): {body}"));
        }

        let parsed: AnthropicResponse = response
            .json()
            .await
            .map_err(|err| format!("failed to decode Claude response: {err}"))?;

        let answer = parsed
            .content
            .iter()
            .filter_map(|block| {
                if block.kind == "text" {
                    block.text.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let usage = parsed.usage.map(|u| ClaudeUsage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        });

        Ok(ClaudeQuestionResponse {
            answer,
            model: parsed.model,
            stop_reason: parsed.stop_reason,
            usage,
        })
    }

    async fn ask_claude_voice(
        self,
        request: ClaudeVoiceRequest,
    ) -> Result<ClaudeVoiceResponse, String> {
        if request.api_key.trim().is_empty() {
            return Err("Claude API key is required".to_string());
        }
        if request.audio_base64.trim().is_empty() {
            return Err("Recorded audio is required".to_string());
        }

        let audio_format = request
            .audio_format
            .clone()
            .unwrap_or_else(|| "webm".to_string());

        let mut content = Vec::new();
        if let Some(ctx) = request.code_context.as_ref() {
            if !ctx.trim().is_empty() {
                content.push(json!({
                    "type": "text",
                    "text": format!("Context:\n{}", ctx),
                }));
            }
        }
        content.push(json!({
            "type": "input_audio",
            "audio": [
                {
                    "type": "base64",
                    "data": request.audio_base64,
                    "format": audio_format,
                }
            ],
        }));

        if let Some(transcript_hint) = request.transcript_hint.as_ref() {
            if !transcript_hint.trim().is_empty() {
                content.push(json!({
                    "type": "text",
                    "text": format!(
                        "Transcription hint:\n{}\nPlease verify and use the audio for accuracy.",
                        transcript_hint
                    ),
                }));
            }
        }

        let voice = request.voice.clone().unwrap_or_else(|| "verse".to_string());

        let mut body = json!({
            "model": request
                .model
                .clone()
                .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string()),
            "max_output_tokens": request.max_output_tokens.unwrap_or(800),
            "temperature": request.temperature.unwrap_or(0.2),
            "messages": [
                {
                    "role": "user",
                    "content": content,
                }
            ],
            "betas": [
                "audio_input_2024-10-22",
                "audio_output_2024-10-22"
            ],
            "response_format": [
                {"type": "text"},
                {
                    "type": "audio",
                    "audio": {
                        "voice": voice,
                        "format": "wav"
                    }
                }
            ],
        });

        if let Some(system) = request.system_prompt.as_ref() {
            if let Some(map) = body.as_object_mut() {
                map.insert(
                    "system".to_string(),
                    serde_json::Value::String(system.clone()),
                );
            }
        }

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", request.api_key)
            .header("anthropic-version", "2023-06-01")
            .header(
                "anthropic-beta",
                "audio_input_2024-10-22,audio_output_2024-10-22",
            )
            .json(&body)
            .send()
            .await
            .map_err(|err| format!("failed to call Claude: {err}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Claude API error ({status}): {body}"));
        }

        let parsed: Value = response
            .json()
            .await
            .map_err(|err| format!("failed to decode Claude response: {err}"))?;

        let mut answer_text = String::new();
        let mut audio_base64: Option<String> = None;
        let mut audio_mime = String::from("audio/wav");

        if let Some(content) = parsed.get("content").and_then(|v| v.as_array()) {
            for block in content {
                match block.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            if !answer_text.is_empty() {
                                answer_text.push_str("\n\n");
                            }
                            answer_text.push_str(text);
                        }
                    }
                    Some("output_audio") => {
                        if let Some(audio_obj) = block.get("audio") {
                            if let Some(format) = audio_obj.get("format").and_then(|v| v.as_str()) {
                                audio_mime = audio_format_to_mime(format);
                            }
                            if let Some(data) = audio_obj.get("data").and_then(|v| v.as_str()) {
                                audio_base64 = Some(data.to_string());
                            } else if let Some(chunks) =
                                audio_obj.get("audio").and_then(|v| v.as_array())
                            {
                                for chunk in chunks {
                                    if chunk.get("type").and_then(|v| v.as_str()) == Some("base64")
                                    {
                                        if let Some(data) =
                                            chunk.get("data").and_then(|v| v.as_str())
                                        {
                                            audio_base64 = Some(data.to_string());
                                            if let Some(format) =
                                                chunk.get("format").and_then(|v| v.as_str())
                                            {
                                                audio_mime = audio_format_to_mime(format);
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let usage = parsed.get("usage").and_then(|usage| {
            Some(ClaudeUsage {
                input_tokens: usage
                    .get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default() as u32,
                output_tokens: usage
                    .get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default() as u32,
            })
        });

        let transcript = parsed
            .get("metadata")
            .and_then(|meta| meta.get("input_transcript"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                parsed
                    .get("content")
                    .and_then(|v| v.as_array())
                    .and_then(|blocks| {
                        blocks.iter().find_map(|block| {
                            if block.get("type").and_then(|v| v.as_str()) == Some("transcript") {
                                return block
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                            }
                            None
                        })
                    })
            });

        Ok(ClaudeVoiceResponse {
            answer_text,
            answer_audio_base64: audio_base64,
            answer_audio_mime_type: Some(audio_mime),
            transcript,
            model: parsed
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            stop_reason: parsed
                .get("stop_reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            usage,
        })
    }

    async fn generate_livekit_token(
        self,
        request: LivekitTokenRequest,
    ) -> Result<LivekitTokenResponse, String> {
        let ttl = request.ttl_seconds.unwrap_or(3600);
        if ttl <= 0 {
            return Err("TTL must be positive".to_string());
        }

        let now = Utc::now();
        let expires_at = now
            .checked_add_signed(TimeDelta::seconds(ttl))
            .ok_or_else(|| "ttl results in overflow".to_string())?;

        let grant = LivekitVideoGrant {
            room_join: true,
            room: request.room.clone(),
            can_publish: request.can_publish,
            can_publish_data: request.can_publish_data,
            can_subscribe: request.can_subscribe,
        };

        let claims = LivekitClaims {
            iss: request.api_key.clone(),
            sub: request.identity.clone(),
            name: request.name.clone(),
            metadata: request.metadata.clone(),
            exp: expires_at.timestamp(),
            nbf: (now - TimeDelta::seconds(30)).timestamp(),
            aud: Some("video".to_string()),
            video: grant,
        };

        let header = Header::new(Algorithm::HS256);
        let token = jsonwebtoken::encode(
            &header,
            &claims,
            &EncodingKey::from_secret(request.api_secret.as_bytes()),
        )
        .map_err(|err| format!("failed to sign token: {err}"))?;

        Ok(LivekitTokenResponse {
            token,
            expires_at: expires_at.to_rfc3339(),
        })
    }
}

fn audio_format_to_mime(format: &str) -> String {
    match format.to_ascii_lowercase().as_str() {
        "wav" | "wave" => "audio/wav".to_string(),
        "mp3" => "audio/mpeg".to_string(),
        "ogg" | "oga" => "audio/ogg".to_string(),
        "webm" => "audio/webm".to_string(),
        other => format!("audio/{}", other),
    }
}

mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(taurpc::create_ipc_handler(
            ApiImpl { client }.into_handler(),
        ))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
