// Critic Agent

use crate::core::error::{AeraError, AeraResult};
use crate::infrastructure::gemini::{
    Content, GeminiClient, GeminiRequest, GenerationConfig, Part,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum CriticVerdict {
    /// Taslak yeterli
    Pass,
    /// Taslak yetersiz/hatalı (Düzeltilmiş cevap içerir)
    Revise {
        issues: Vec<String>,
        improved_answer: String,
    },
}

impl CriticVerdict {
    /// Log etiketi
    pub fn label(&self) -> &'static str {
        match self {
            CriticVerdict::Pass => "PASS",
            CriticVerdict::Revise { .. } => "REVISE",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct CriticRawResponse {
    verdict: String,
    #[serde(default)]
    issues: Vec<String>,
    #[serde(default)]
    improved_answer: Option<String>,
}

const CRITIC_SYSTEM_PROMPT: &str = r#"Sen AeraCFO Critic Agent'sın. Executor'un ürettiği cevabı denetlersin.

## Görevin
Verilen (kullanıcı sorusu, draft cevap, çalıştırılan tool'lar) üçlüsünü incele. Cevap kabul edilebilir mi karar ver.

## Kabul Kriterleri (HEPSİ doğru olmalı, biri eksikse REVISE)
1. **Doğruluk**: Sayılar tutarlı mı? Tool çağrıldı ama sayı atlanmış mı?
2. **Tamlık**: Kullanıcı sorduğu her şeye cevap var mı?
3. **Format**: TL tutarları 45.320,00 TL stilinde mi? Yüzdeler 1 ondalıkla mı?
4. **Eylem**: Analitik sorularda somut tavsiye var mı (sadece veri dökümü değil)?
5. **[AERA_METRICS] / [AERA_INCENTIVES] blokları**: Tool çağrıldıysa ilgili dashboard bloğu mevcut mu?
6. **Halüsinasyon yok**: Çağrılmamış tool'un çıktısı uydurulmamış mı?

## Çıktı Formatı (SADECE GEÇERLI JSON)

PASS durumunda:
{"verdict": "PASS", "issues": [], "improved_answer": null}

REVISE durumunda:
{
  "verdict": "REVISE",
  "issues": ["Eksik 1: ...", "Eksik 2: ..."],
  "improved_answer": "Düzeltilmiş tam cevap METNİ buraya. Dashboard blokları korunmalı."
}

## Kurallar
- improved_answer kullanıcıya doğrudan gidecek — TR, profesyonel ama sıcak
- Draft içindeki [AERA_METRICS]...[/AERA_METRICS] bloklarını AYNEN koru, sayıları değiştirme
- improved_answer 1500 karakteri geçmesin
- Sadece JSON, açıklama yazma
- Sohbet sorularında (selamlama, "ne yapabilirsin") draft genelde PASS"#;

/// Critic isteği başlatır
pub async fn critique(
    gemini: &GeminiClient,
    user_message: &str,
    draft_answer: &str,
    tools_used: &[String],
) -> AeraResult<CriticVerdict> {
    // Sohbet sorularında Critic pas geçilir
    if tools_used.is_empty() && draft_answer.len() < 500 {
        return Ok(CriticVerdict::Pass);
    }

    let tools_line = if tools_used.is_empty() {
        "(hiçbir tool çağrılmadı)".to_string()
    } else {
        tools_used.join(", ")
    };

    let user_content = Content {
        role: "user".to_string(),
        parts: vec![Part::Text {
            text: format!(
                "KULLANICI SORUSU:\n{user_message}\n\n---\n\nDRAFT CEVAP:\n{draft_answer}\n\n---\n\nÇAĞRILAN TOOL'LAR: {tools_line}\n\nLütfen denetle ve JSON karar ver."
            ),
        }],
    };

    let system = Content {
        role: String::new(),
        parts: vec![Part::Text { text: CRITIC_SYSTEM_PROMPT.to_string() }],
    };

    // Critic deterministik olmalı, yaratıcılık değil tutarlılık hedeflenir.
    let config = GenerationConfig {
        temperature: 0.1,
        top_p: 0.7,
        max_output_tokens: 2048,
        ..GenerationConfig::default()
    };

    let request = GeminiRequest {
        system_instruction: Some(&system),
        contents: std::slice::from_ref(&user_content),
        tools: None,
        generation_config: Some(config),
    };

    let response = gemini.generate(&request).await?;
    let raw_text = extract_text(&response)?;
    parse_critic_response(&raw_text)
}

fn extract_text(response: &serde_json::Value) -> AeraResult<String> {
    let parts = response["candidates"][0]["content"]["parts"]
        .as_array()
        .ok_or_else(|| AeraError::GeminiApiError(
            "Critic: candidates[0].content.parts bulunamadı".into()
        ))?;
    let mut text = String::new();
    for p in parts {
        if let Some(t) = p["text"].as_str() {
            text.push_str(t);
        }
    }
    if text.trim().is_empty() {
        return Err(AeraError::GeminiApiError(
            "Critic: Gemini boş metin döndü".into(),
        ));
    }
    Ok(text)
}

pub fn parse_critic_response(raw: &str) -> AeraResult<CriticVerdict> {
    let stripped = strip_code_fences(raw);
    let json_slice = extract_first_json_object(&stripped)
        .ok_or_else(|| AeraError::AgentExecutionError(
            "Critic JSON bloğu tespit edilemedi".into()
        ))?;

    let parsed: CriticRawResponse = serde_json::from_str(json_slice)
        .map_err(|e| AeraError::AgentExecutionError(
            format!("Critic JSON parse hatası: {}. Ham: {}", e, &json_slice[..json_slice.len().min(200)])
        ))?;

    match parsed.verdict.to_uppercase().as_str() {
        "PASS" => Ok(CriticVerdict::Pass),
        "REVISE" => {
            let improved = parsed.improved_answer
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| AeraError::AgentExecutionError(
                    "Critic REVISE dedi ama improved_answer boş".into()
                ))?;
            Ok(CriticVerdict::Revise {
                issues: parsed.issues,
                improved_answer: improved,
            })
        }
        other => Err(AeraError::AgentExecutionError(
            format!("Critic beklenmedik verdict: '{}'", other)
        )),
    }
}

fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    trimmed.to_string()
}

fn extract_first_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if escape { escape = false; continue; }
        match b {
            b'\\' if in_string => escape = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pass() {
        let raw = r#"{"verdict":"PASS","issues":[],"improved_answer":null}"#;
        let v = parse_critic_response(raw).unwrap();
        assert!(matches!(v, CriticVerdict::Pass));
    }

    #[test]
    fn test_parse_revise() {
        let raw = r#"{"verdict":"REVISE","issues":["TL formatı eksik","Tavsiye yok"],"improved_answer":"Sayın kullanıcı, finansal durumunuz...\n[AERA_METRICS]{}[/AERA_METRICS]"}"#;
        let v = parse_critic_response(raw).unwrap();
        match v {
            CriticVerdict::Revise { issues, improved_answer } => {
                assert_eq!(issues.len(), 2);
                assert!(improved_answer.contains("AERA_METRICS"));
            }
            _ => panic!("REVISE bekleniyordu"),
        }
    }

    #[test]
    fn test_parse_revise_with_empty_answer_rejected() {
        let raw = r#"{"verdict":"REVISE","issues":["x"],"improved_answer":""}"#;
        assert!(parse_critic_response(raw).is_err());
    }

    #[test]
    fn test_parse_with_code_fence() {
        let raw = "```json\n{\"verdict\":\"PASS\"}\n```";
        let v = parse_critic_response(raw).unwrap();
        assert!(matches!(v, CriticVerdict::Pass));
    }

    #[test]
    fn test_parse_unknown_verdict_errors() {
        let raw = r#"{"verdict":"MAYBE","issues":[],"improved_answer":null}"#;
        assert!(parse_critic_response(raw).is_err());
    }

    #[test]
    fn test_verdict_label() {
        assert_eq!(CriticVerdict::Pass.label(), "PASS");
        let r = CriticVerdict::Revise { issues: vec![], improved_answer: "x".into() };
        assert_eq!(r.label(), "REVISE");
    }
}
