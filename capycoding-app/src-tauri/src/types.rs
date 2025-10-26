#[taurpc::ipc_type]
pub struct ClaudeMetricsRequest {
    pub data_dir: Option<String>,
    pub hours_back: Option<u32>,
    pub python_path: Option<String>,
}

#[taurpc::ipc_type]
pub struct ClaudeMetricsSnapshot {
    pub timestamp: String,
    pub window_hours: f64,
    pub burn_rate_per_hour: f64,
    pub total_cost_usd: f64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub total_tokens: i64,
    pub session_count: i32,
    pub active_session_id: Option<String>,
    pub last_activity: String,
    pub source: Option<String>,
}

#[taurpc::ipc_type]
pub struct PushClaudeMetricsRequest {
    pub metrics: ClaudeMetricsSnapshot,
    pub server_url: String,
    pub auth_token: Option<String>,
}

#[taurpc::ipc_type]
pub struct ClaudeQuestionRequest {
    pub api_key: String,
    pub question: String,
    pub code_context: Option<String>,
    pub model: Option<String>,
    pub max_output_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub system_prompt: Option<String>,
}

#[taurpc::ipc_type]
pub struct ClaudeUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[taurpc::ipc_type]
pub struct ClaudeQuestionResponse {
    pub answer: String,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Option<ClaudeUsage>,
}

#[taurpc::ipc_type]
pub struct LivekitTokenRequest {
    pub api_key: String,
    pub api_secret: String,
    pub identity: String,
    pub room: String,
    pub name: Option<String>,
    pub metadata: Option<String>,
    pub ttl_seconds: Option<i64>,
    pub can_publish: Option<bool>,
    pub can_subscribe: Option<bool>,
    pub can_publish_data: Option<bool>,
}

#[taurpc::ipc_type]
pub struct LivekitTokenResponse {
    pub token: String,
    pub expires_at: String,
}
