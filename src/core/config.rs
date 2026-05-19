use crate::core::error::AeraResult;

/// AeraCFO'nun tüm yapılandırma ayarlarını tutan yapı.
/// `.env` dosyasından `dotenvy` ile yüklenir.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Gemini API Anahtarı
    pub gemini_api_key: String,
    /// Sunucu adresi (örn: "0.0.0.0:3000")
    pub server_addr: String,
}

impl AppConfig {
    /// `.env` dosyasından yapılandırmayı yükler.
    pub fn load() -> AeraResult<Self> {
        // .env dosyasını oku (yoksa hata verme, ortam değişkenleri zaten set olabilir)
        dotenvy::dotenv().ok();

        let gemini_api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| crate::core::error::AeraError::EnvVarError("GEMINI_API_KEY".into()))?;

        let server_addr = std::env::var("SERVER_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        Ok(Self {
            gemini_api_key,
            server_addr,
        })
    }
}
