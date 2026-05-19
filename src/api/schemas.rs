use serde::{Deserialize, Serialize};

// REQUEST SCHEMAS

/// Sohbet isteği
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: Option<String>,
}

/// CSV yükleme isteği
#[derive(Debug, Deserialize)]
pub struct UploadRequest {
    pub session_id: String,
    pub income_column: Option<String>,
    pub expense_column: Option<String>,
    pub date_column: Option<String>,
}

// RESPONSE SCHEMAS

/// Sohbet yanıtı
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub reply: String,
    pub tools_used: Vec<String>,
    pub latency_ms: u64,
    pub session_id: String,
    pub agent_trace: crate::agents::coordinator::AgentTrace,
}

/// Health-check yanıtı
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub engine: &'static str,
    pub active_sessions: usize,
}

/// Yükleme yanıtı
#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub success: bool,
    pub message: String,
    pub rows: usize,
    pub columns: usize,
    pub column_names: Vec<String>,
    pub date_range: Option<DateRange>,
    pub monthly_data: Vec<serde_json::Value>,
}

/// Tarih aralığı
#[derive(Debug, Clone, Serialize)]
pub struct DateRange {
    pub start: String,
    pub end: String,
    pub days: i64,
}

/// Hata yanıtı
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub title: String,
    pub detail: String,
}
