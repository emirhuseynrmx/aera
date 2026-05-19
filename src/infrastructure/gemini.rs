use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::core::error::{AeraError, AeraResult};

/// Gemini API istemcisi
pub struct GeminiClient {
    client: Client,
    api_key: String,
    model: String,
}

/// Gemini 2.5 Flash thinking modunu kapatır — aksi halde reasoning tokenları
/// max_output_tokens'i yiyip `finishReason: STOP` + boş parts üretiyor.
#[derive(Debug, Clone, Serialize)]
pub struct ThinkingConfig {
    #[serde(rename = "thinkingBudget")]
    pub thinking_budget: u32,
}

/// Düşük temperature → deterministik, tutarlı finansal çıktı
#[derive(Debug, Clone, Serialize)]
pub struct GenerationConfig {
    pub temperature: f32,
    #[serde(rename = "topP")]
    pub top_p: f32,
    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: u32,
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.3,
            top_p: 0.8,
            max_output_tokens: 8192,
            thinking_config: Some(ThinkingConfig { thinking_budget: 0 }),
        }
    }
}

/// Gemini Request Payload
#[derive(Debug, Serialize)]
pub struct GeminiRequest<'a> {
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<&'a Content>,
    pub contents: &'a [Content],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<&'a [Tool]>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    // role boş ise serialize edilmez — systemInstruction için role'süz Content kullanılabilir
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    Text { text: String },
    FunctionCall { function_call: FunctionCall },
    FunctionResponse { function_response: FunctionResponse },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    pub function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl GeminiClient {
    pub fn new(api_key: String) -> Self {
        // Client pooling
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .expect("reqwest client oluşturulamadı — TLS backend eksik olabilir");
        let model = std::env::var("GEMINI_MODEL")
            .unwrap_or_else(|_| "gemini-2.5-flash".to_string());
        tracing::info!("🤖 Gemini model: {}", model);
        Self {
            client,
            api_key,
            model,
        }
    }

    /// API anahtarı güncelleme
    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = api_key;
    }

    /// Exponential backoff ile istek atar
    pub async fn generate(&self, request: &GeminiRequest<'_>) -> AeraResult<serde_json::Value> {
        let base_delays = [1000u64, 2000, 4000, 8000];

        for (attempt, &base_delay) in base_delays.iter().enumerate() {
            match self.try_generate(request).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    let err_msg = e.to_string();
                    // 5xx gateway hataları + 429 rate-limit + RETRYABLE_EMPTY (thinking modu STOP'u)
                    let retryable = err_msg.contains("503")
                        || err_msg.contains("429")
                        || err_msg.contains("500")
                        || err_msg.contains("502")
                        || err_msg.contains("504")
                        || err_msg.contains("RETRYABLE_EMPTY");
                    if retryable {
                        // 429 ise Google'ın söylediği retryDelay'i kullan, yoksa exponential backoff
                        let delay = parse_retry_delay_ms(&err_msg).unwrap_or(base_delay).min(35_000);
                        tracing::warn!(
                            "Gemini API yoğunluk hatası ({}). Deneme {}/5 başarısız. {}ms sonra tekrar...",
                            err_msg, attempt + 1, delay
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    } else {
                        // Auth/400 gibi retryable olmayan hatalar — anında dön, kullanıcıyı bekletme
                        return Err(e);
                    }
                }
            }
        }

        // 5'inci ve son deneme — başarısızsa gerçek hata yukarı çıkar
        self.try_generate(request).await
    }

    async fn try_generate(&self, request: &GeminiRequest<'_>) -> AeraResult<serde_json::Value> {
        // Key header'da gönderilir — URL'de olursa HTTP log'larına ve CDN cache'lerine düşer
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );

        let response = self.client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AeraError::GeminiApiError(
                format!("HTTP {}: {}", status.as_u16(), error_body)
            ));
        }

        // Yanıt şemasını defensif olarak parse et — API format değişikliğine karşı
        let body: serde_json::Value = response.json().await
            .map_err(|e| AeraError::GeminiApiError(
                format!("Gemini yanıtı JSON parse edilemedi: {}", e)
            ))?;

        // Beklenmedik boş yanıt — erken hata ver, downstream panic yerine
        if !body["candidates"].is_array() && body["promptFeedback"].is_null() {
            return Err(AeraError::GeminiApiError(
                "Gemini beklenmedik format döndürdü (candidates yok).".into()
            ));
        }

        // Boş parts + STOP/MAX_TOKENS → thinking modu reasoning'e harcayıp output bırakmamış olabilir.
        // RETRYABLE_EMPTY sentinel'ı ile generate() retry'a sokar — 5 deneme şansı verir.
        if let Some(candidates) = body["candidates"].as_array() {
            if let Some(first) = candidates.first() {
                let parts_empty = first["content"]["parts"]
                    .as_array()
                    .is_none_or(|p| p.is_empty());
                let finish_reason = first["finishReason"].as_str().unwrap_or("");
                if parts_empty
                    && (finish_reason == "STOP"
                        || finish_reason == "MAX_TOKENS"
                        || finish_reason == "OTHER"
                        || finish_reason.is_empty())
                {
                    return Err(AeraError::GeminiApiError(format!(
                        "RETRYABLE_EMPTY: Gemini boş content döndürdü (finishReason: {})",
                        finish_reason
                    )));
                }
            }
        }

        Ok(body)
    }
}

// 429 yanıtında Google "retryDelay": "31s" veya "Please retry in 31.7s" diyor.
// İkisinden birini yakalayıp ms cinsinden döndürür. Bulamazsa None.
fn parse_retry_delay_ms(err_msg: &str) -> Option<u64> {
    // Önce structured field: "retryDelay":"31s"
    const KEY: &str = "\"retryDelay\"";
    if let Some(idx) = err_msg.find(KEY) {
        let rest = &err_msg[idx + KEY.len()..];
        if let Some(start_q) = rest.find('"') {
            let val_part = &rest[start_q + 1..];
            if let Some(end_q) = val_part.find('"') {
                let val = &val_part[..end_q];
                if let Some(secs) = val.strip_suffix('s').and_then(|s| s.parse::<f64>().ok()) {
                    return Some(((secs + 1.0) * 1000.0) as u64); // +1s emniyet payı
                }
            }
        }
    }
    // Fallback: "Please retry in 31.76s"
    if let Some(idx) = err_msg.find("retry in ") {
        let tail = &err_msg[idx + "retry in ".len()..];
        let num: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if let Ok(secs) = num.parse::<f64>() {
            return Some(((secs + 1.0) * 1000.0) as u64);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_retry_delay() {
        let msg = r#"HTTP 429: {"error":{"details":[{"@type":"...RetryInfo","retryDelay":"31s"}]}}"#;
        assert_eq!(parse_retry_delay_ms(msg), Some(32_000));
    }

    #[test]
    fn parses_text_retry_delay() {
        let msg = "Please retry in 24.5s.";
        assert_eq!(parse_retry_delay_ms(msg), Some(25_500));
    }

    #[test]
    fn returns_none_when_absent() {
        assert_eq!(parse_retry_delay_ms("generic error"), None);
    }
}
