use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Tüm sistem hataları. thiserror → otomatik Display.
#[derive(Debug, Error)]
pub enum AeraError {
    #[error("Çevresel değişken bulunamadı: {0}")]
    EnvVarError(String),

    #[error("Gemini AI API Hatası: {0}")]
    GeminiApiError(String),

    #[error("Serileştirme/JSON Hatası: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Ajan (Agent) İşlem Hatası: {0}")]
    AgentExecutionError(String),

    #[error("Geçersiz İstek: {0}")]
    BadRequest(String),

    #[error("Yetkisiz: {0}")]
    Unauthorized(String),

    #[error("Çok fazla istek: {0}")]
    TooManyRequests(String),

    #[error("HTTP İstemci Hatası: {0}")]
    HttpClientError(#[from] reqwest::Error),
}

/// Axum AeraError'u otomatik JSON HTTP yanıtına çevirir.
impl IntoResponse for AeraError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AeraError::BadRequest(_)      => (StatusCode::BAD_REQUEST,           self.to_string()),
            AeraError::Unauthorized(_)    => (StatusCode::UNAUTHORIZED,           self.to_string()),
            AeraError::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS,      self.to_string()),
            AeraError::EnvVarError(_)     => (StatusCode::INTERNAL_SERVER_ERROR,  self.to_string()),
            AeraError::GeminiApiError(msg) if msg.contains("429") || msg.contains("RESOURCE_EXHAUSTED") || msg.contains("quota") => (
                StatusCode::TOO_MANY_REQUESTS,
                "⏳ API kotanız geçici olarak doldu (Gemini free-tier: dakikada 10 istek). Yaklaşık 30 saniye bekleyip tekrar deneyin, ya da farklı bir Google projesinde yeni bir API anahtarı oluşturun.".to_string(),
            ),
            AeraError::GeminiApiError(msg) if msg.contains("403") || msg.contains("PERMISSION_DENIED") || msg.contains("denied access") => (
                StatusCode::FORBIDDEN,
                "🔑 API anahtarınız reddedildi. Anahtar yanlış olabilir veya Google projesi engellenmiş. aistudio.google.com/app/apikey adresinden YENİ bir projede yeni bir anahtar üretin.".to_string(),
            ),
            AeraError::GeminiApiError(msg) if msg.contains("401") || msg.contains("UNAUTHENTICATED") || msg.contains("API key not valid") => (
                StatusCode::UNAUTHORIZED,
                "🔑 API anahtarı geçersiz. Ayarlardan doğru anahtarı girdiğinizden emin olun.".to_string(),
            ),
            AeraError::GeminiApiError(msg) if msg.contains("503") || msg.contains("UNAVAILABLE") || msg.contains("502") || msg.contains("504") || msg.contains("500") => (
                StatusCode::SERVICE_UNAVAILABLE,
                "⏳ Gemini sunucusu şu an yoğun (geçici Google tarafı arızası). Birkaç saniye bekleyip tekrar deneyin.".to_string(),
            ),
            AeraError::GeminiApiError(msg) if msg.contains("RETRYABLE_EMPTY") || msg.contains("boş yanıt") || msg.contains("boş content") => (
                StatusCode::BAD_GATEWAY,
                "🤔 Yapay zeka şu an cevap üretemedi. Genelde sebebi: (1) önce demo sektörü seçmeden veri gerektiren soru sorulmuş, (2) Gemini özetleme katmanı tıkanmış. Sol panelden bir demo seçip tekrar deneyin.".to_string(),
            ),
            AeraError::GeminiApiError(msg) if msg.contains("blockReason") || msg.contains("SAFETY") => (
                StatusCode::BAD_REQUEST,
                "🚫 Gemini güvenlik filtresi bu soruyu engelledi. Lütfen soruyu yeniden ifade edin.".to_string(),
            ),
            AeraError::GeminiApiError(_)  => (
                StatusCode::BAD_GATEWAY,
                "⚠️ Gemini bağlantısında beklenmedik bir hata oldu. Birkaç saniye sonra tekrar deneyin.".to_string(),
            ),
            _                             => (StatusCode::INTERNAL_SERVER_ERROR,  self.to_string()),
        };

        let body = Json(json!({
            "error": {
                "kind": status.as_u16(),
                "message": error_message
            }
        }));

        (status, body).into_response()
    }
}

pub type AeraResult<T> = std::result::Result<T, AeraError>;