// Planner Agent

use crate::core::error::{AeraError, AeraResult};
use crate::infrastructure::gemini::{Content, GeminiClient, GeminiRequest, GenerationConfig, Part};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: u32,
    pub question: String,
    /// Önerilen araçlar
    #[serde(default)]
    pub suggested_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResult {
    /// Strateji özeti
    pub strategy: String,
    pub subtasks: Vec<SubTask>,
    /// Araç gereksinim durumu
    pub requires_tools: bool,
}

const PLANNER_SYSTEM_PROMPT: &str = r#"Sen AeraCFO Planner Agent'sın. Görevin tek başına cevap üretmek DEĞİL; kullanıcının sorusunu inceleyip Executor Agent'ın izleyeceği yol haritasını JSON olarak çıkarmak.

## Mevcut Tool Envanteri (Executor bunları çağıracak)
- get_data_summary: Veri yüklü mü ve şeması ne
- get_health_score: 0-100 finansal sağlık skoru
- analyze_cash_flow: Net nakit pozisyonu, burn rate, runway
- predict_cashflow: 1-6 ay ileriye Holt çift üstel tahmin
- detect_anomalies: Z-score ile anormal ay tespiti
- detect_cash_crunch: Ardışık negatif ay riski
- analyze_expense_categories: Kategori bazlı gider dağılımı
- compare_sector_benchmark: Türkiye KOBİ sektör kıyası
- simulate_scenario: What-if (ek gelir/gider, X kişi alsam)
- search_incentives: KOSGEB/TÜBİTAK teşvik araması

## Görev Şablonu
Soruyu 1-4 alt göreve böl. Her görev bağımsız ve net olmalı. Çakışan tool'ları tekrarlama.

## Çıktı Formatı (SADECE GEÇERLI JSON, ek metin YOK)
{
  "strategy": "Tek cümle plan özeti",
  "requires_tools": true|false,
  "subtasks": [
    {"id": 1, "question": "Alt soru cümlesi", "suggested_tools": ["tool_a", "tool_b"]}
  ]
}

## Kurallar
- "Merhaba", "nedir", "nasıl çalışır" gibi sohbet sorularında: requires_tools=false, tek subtask, boş suggested_tools
- Sayısal/analitik sorularda: requires_tools=true, minimum gerekli tool'ları seç (over-engineering YOK)
- Veri YÜKLENMEMİŞSE ve soru veri gerektiriyorsa: tek subtask "Kullanıcıya CSV yüklemesini iste", requires_tools=false
- subtask sayısı 1-4 arası, fazlası executor'u yavaşlatır
- Sadece JSON döndür, açıklama yazma"#;

/// Planner'ı Gemini ile çalıştırır. Tek çağrı, düşük token.
pub async fn plan(
    gemini: &GeminiClient,
    user_message: &str,
    data_loaded: bool,
) -> AeraResult<PlanResult> {
    let context_line = if data_loaded {
        "BAĞLAM: Kullanıcının CSV verisi YÜKLÜ — finansal tool'lar kullanılabilir."
    } else {
        "BAĞLAM: Henüz CSV verisi YÜKLENMEMİŞ — finansal tool'lar veri olmadan çalışmaz."
    };
    let user_content = Content {
        role: "user".to_string(),
        parts: vec![Part::Text {
            text: format!("{}\n\nKULLANICI SORUSU:\n{}", context_line, user_message),
        }],
    };
    let system = Content {
        role: String::new(),
        parts: vec![Part::Text {
            text: PLANNER_SYSTEM_PROMPT.to_string(),
        }],
    };

    // Planner sıcaklığı düşük — yaratıcı plan yerine deterministik şema hedeflenir.
    let config = GenerationConfig {
        temperature: 0.15,
        top_p: 0.8,
        max_output_tokens: 600,
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
    parse_plan_response(&raw_text)
}

fn extract_text(response: &serde_json::Value) -> AeraResult<String> {
    let parts = response["candidates"][0]["content"]["parts"]
        .as_array()
        .ok_or_else(|| {
            AeraError::GeminiApiError("Planner: candidates[0].content.parts bulunamadı".into())
        })?;
    let mut text = String::new();
    for p in parts {
        if let Some(t) = p["text"].as_str() {
            text.push_str(t);
        }
    }
    if text.trim().is_empty() {
        return Err(AeraError::GeminiApiError(
            "Planner: Gemini boş metin döndü".into(),
        ));
    }
    Ok(text)
}

/// Gemini'nin JSON dönmesi garanti değildir; bazen ```json fences, bazen
/// önüne "İşte plan:" gibi açıklama ekliyor. Önce fence sıyır, sonra ilk { ile son }
/// arasındaki bloğu çek. Yine de hata olursa kullanıcıya düşmesin — fallback plan üret.
pub fn parse_plan_response(raw: &str) -> AeraResult<PlanResult> {
    let stripped = strip_code_fences(raw);
    let json_slice = extract_first_json_object(&stripped).ok_or_else(|| {
        AeraError::AgentExecutionError("Planner JSON bloğu tespit edilemedi".into())
    })?;

    let plan: PlanResult = serde_json::from_str(json_slice).map_err(|e| {
        AeraError::AgentExecutionError(format!(
            "Planner JSON parse hatası: {}. Ham: {}",
            e,
            &json_slice[..json_slice.len().min(200)]
        ))
    })?;

    if plan.subtasks.is_empty() {
        return Err(AeraError::AgentExecutionError(
            "Planner boş subtask listesi döndü".into(),
        ));
    }
    // 4'ten fazla subtask Executor'da round limitini aşar — emniyet kemeri
    if plan.subtasks.len() > 4 {
        return Err(AeraError::AgentExecutionError(format!(
            "Planner çok fazla subtask döndü: {}. Soruyu basitleştirin.",
            plan.subtasks.len()
        )));
    }
    Ok(plan)
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
        if escape {
            escape = false;
            continue;
        }
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

/// Planner çağrısı başarısız olursa Coordinator graceful degrade etsin diye fallback.
/// Tek subtask = kullanıcı sorusunun kendisi, tool önerisi yok.
pub fn fallback_plan(user_message: &str) -> PlanResult {
    PlanResult {
        strategy: "Planner devre dışı — soru doğrudan Executor'a iletiliyor".to_string(),
        requires_tools: true,
        subtasks: vec![SubTask {
            id: 1,
            question: user_message.to_string(),
            suggested_tools: vec![],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clean_json() {
        let raw = r#"{"strategy":"Nakit analizi","requires_tools":true,"subtasks":[{"id":1,"question":"Nakit durumu?","suggested_tools":["get_health_score"]}]}"#;
        let plan = parse_plan_response(raw).unwrap();
        assert_eq!(plan.subtasks.len(), 1);
        assert_eq!(plan.subtasks[0].suggested_tools, vec!["get_health_score"]);
        assert!(plan.requires_tools);
    }

    #[test]
    fn test_parse_with_code_fences() {
        let raw = "```json\n{\"strategy\":\"x\",\"requires_tools\":false,\"subtasks\":[{\"id\":1,\"question\":\"merhaba\",\"suggested_tools\":[]}]}\n```";
        let plan = parse_plan_response(raw).unwrap();
        assert!(!plan.requires_tools);
        assert_eq!(plan.subtasks[0].question, "merhaba");
    }

    #[test]
    fn test_parse_with_leading_text() {
        let raw = "İşte plan:\n{\"strategy\":\"a\",\"requires_tools\":true,\"subtasks\":[{\"id\":1,\"question\":\"q1\",\"suggested_tools\":[\"t1\"]}]}\nUmarım yardımcı olur.";
        let plan = parse_plan_response(raw).unwrap();
        assert_eq!(plan.strategy, "a");
    }

    #[test]
    fn test_rejects_empty_subtasks() {
        let raw = r#"{"strategy":"x","requires_tools":true,"subtasks":[]}"#;
        assert!(parse_plan_response(raw).is_err());
    }

    #[test]
    fn test_rejects_too_many_subtasks() {
        let raw = r#"{"strategy":"x","requires_tools":true,"subtasks":[
            {"id":1,"question":"a","suggested_tools":[]},
            {"id":2,"question":"b","suggested_tools":[]},
            {"id":3,"question":"c","suggested_tools":[]},
            {"id":4,"question":"d","suggested_tools":[]},
            {"id":5,"question":"e","suggested_tools":[]}
        ]}"#;
        assert!(parse_plan_response(raw).is_err());
    }

    #[test]
    fn test_extract_json_handles_nested_strings() {
        let raw = r#"{"strategy":"with {brace} in string","requires_tools":true,"subtasks":[{"id":1,"question":"x","suggested_tools":[]}]}"#;
        let plan = parse_plan_response(raw).unwrap();
        assert!(plan.strategy.contains("{brace}"));
    }

    #[test]
    fn test_fallback_plan() {
        let p = fallback_plan("test sorusu");
        assert_eq!(p.subtasks.len(), 1);
        assert_eq!(p.subtasks[0].question, "test sorusu");
    }
}
