use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::types::{
    ClaudeMetricsRequest, ClaudeMetricsSnapshot, ClaudeQuestionRequest, ClaudeQuestionResponse,
    ClaudeUsage, LivekitTokenRequest, LivekitTokenResponse, PushClaudeMetricsRequest,
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
