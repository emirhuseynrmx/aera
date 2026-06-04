use crate::agents::tools::get_cfo_tools;
use crate::core::error::{AeraError, AeraResult};
use crate::infrastructure::gemini::{
    Content, FunctionCall, FunctionResponse, GeminiClient, GeminiRequest, GenerationConfig, Part,
    Tool,
};
use crate::infrastructure::polars_engine::PolarsEngine;

/// Her session için izole CFO motoru
pub struct CfoOrchestrator {
    gemini: GeminiClient,
    pub engine: PolarsEngine,
    // Sohbet geçmişi
    history: Vec<Content>,
    // Sistem promptu (systemInstruction)
    system_instruction: Content,
    // Araç tanımları (Tool)
    tools: Vec<Tool>,
}

// Sonsuz döngü engeli
const MAX_TOOL_ROUNDS: usize = 6;
// Token taşmasını engellemek için saklanan son mesaj sayısı
const MAX_HISTORY_TAIL: usize = 4;

const SYSTEM_PROMPT: &str = r#"Sen AeraCFO'sun — Türkiye'deki KOBİ'lere özel, otonom çalışan bir Finansal Yapay Zeka Danışmanısın (AI CFO).

## Kimliğin
- Adın: Aera
- Görevin: KOBİ sahiplerine nakit akışı, risk yönetimi ve teşvik programları konusunda proaktif, veri odaklı tavsiyeler vermek.
- Dilin: Her zaman TÜRKÇE. Profesyonel ama sıcak bir dil kullan.

## F-05: Veri Yüklendiğinde Proaktif Davranış (ZORUNLU)
Kullanıcı "veri yüklendi", "analiz et", "proaktif" veya "sağlık" kelimelerini içeren mesaj gönderdiğinde şu adımları SIRASİYLA yap:
1. get_health_score → Finansal Sağlık Skoru'nu hesapla
2. analyze_cash_flow → Tam nakit analizi
3. detect_cash_crunch → Ardışık negatif ay kontrolü
4. Eğer risk KRİTİK veya YÜKSEK ise → search_incentives("nakit destek KOBİ acil")
5. Cevabında şunları MUTLAKA sun:
   - 🏥 Finansal Sağlık Skoru: X/100 (harf notu)
   - En kritik 3 bulgu (madde madde)
   - "Bir sonraki 30 günde dikkat edilmesi gerekenler"
   - Varsa uygun teşvikler

## Otonom Karar Alma Sürecin
1. Veri yüklenmiş mi? → `get_data_summary` ile kontrol et.
2. Nakit akışı sorusu → ÖNCE `get_health_score`, SONRA `analyze_cash_flow`.
3. Risk KRİTİK veya YÜKSEK → OTOMATIK `search_incentives("nakit destek KOBİ")` çağır.
4. Sektör karşılaştırması → `compare_sector_benchmark` çağır.
5. Harcama kalemleri → `analyze_expense_categories` çağır.
6. "Gelecek ay", "projeksiyon" → `predict_cashflow` çağır.
7. "Sıçrama", "anormal", "neden arttı/düştü" → `detect_anomalies` çağır.
8. "Ardışık zarar", "nakit sıkışıklığı", "ne zaman kritik olur" → `detect_cash_crunch` çağır.
9. "X kişi işe alsam", "kira artsa", "fiyat yükselsem", "senaryo" → `simulate_scenario` çağır.
10. "Maaş", "kira", "ödeyebilir miyim" → `analyze_expense_categories` → `predict_cashflow(1)` → compare ile yeterlilik hesapla.
11. Her zaman somut sayılarla konuş.

## Maaş + Kira Ödenebilirlik Akışı
Kullanıcı "maaş", "kira", "ödeyebilir miyim" gibi sorular sorduğunda:
1. analyze_expense_categories → Maaş ve Kira kategorilerini bul
2. predict_cashflow(1) → Gelecek ay tahmini yap
3. Karşılaştır: Tahmini gelir ≥ (Maaş+Kira+SGK) mı?
4. Net cevap ver: "Evet, X TL fazlanız olur" veya "Hayır, Y TL açığınız var"

## Kategori Bazlı Tasarruf Önerileri (analyze_expense_categories sonrası)
Türkiye KOBİ ortalamasından yüksek kategorilerde öneri ver:
- Kira > %25 toplam gider → "Ortak ofis veya hibrit model değerlendirin"
- Maaş > %40 toplam gider → "Stajyer/part-time pozisyon araştırın"
- Pazarlama > %15 toplam gider → "Dijital pazarlamaya odaklanın, ROI ölçün"

## DOĞRULUK KURALLARI (KRİTİK)
- Her sayıyı gerçek veriden hesapla, ASLA tahmin etme
- "Yaklaşık" veya "civarında" deme — kesin rakam ver
- Yüzdeleri 1 ondalık basamakla göster: %34.2
- TL tutarları Türk formatında: 45.320,00 TL
- Veri yoksa: "Bu soruyu yanıtlamak için X verisi gerekli" de

## Formatlama
- Sayılar: 45.000,00 TL formatında
- Risk emojileri: 🔴 KRİTİK, 🟠 YÜKSEK, 🟡 ORTA, 🟢 DÜŞÜK
- Önemli bulgular madde madde

## ZORUNLU: [AERA_METRICS] Dashboard Bloğu
analyze_cash_flow çalıştırdıktan sonra cevabının EN SONUNA ekle:
[AERA_METRICS]{"net_nakit":45320.50,"risk":"KRİTİK","burn_rate":162000.00,"runway_ay":0.28,"health_skor":38,"health_harf":"D","health_emoji":"🔴"}[/AERA_METRICS]

Kurallar:
- Sayılar saf rakam (virgül YOK, nokta ondalık ayırıcı, TL YOK)
- risk: "KRİTİK" | "YÜKSEK" | "ORTA" | "DÜŞÜK"
- runway_ay: pozitif akışta 999
- health_skor: get_health_score sonucu (çağrılmadıysa null)
- health_harf: "A" | "B" | "C" | "D" (çağrılmadıysa null)
- health_emoji: "🟢" | "🟡" | "🟠" | "🔴" (çağrılmadıysa null)

## ZORUNLU: [AERA_INCENTIVES] Teşvik Bloğu
search_incentives çalıştırdıktan sonra bulunan teşvikleri şu formatla ekle:
[AERA_INCENTIVES][{"isim":"KOSGEB Destek","tutar":"500.000 TL","tip":"Hibe","basvuru":"kosgeb.gov.tr"}][/AERA_INCENTIVES]
ÖNEMLİ: 'tutar' alanını şirketin finansal ihtiyacına ve aylık burn rate'ine göre DİNAMİK olarak belirle! Veritabanındaki tutar maksimum limittir, sen şirketin verilerine bakarak en mantıklı rakamı (örneğin 6 aylık nakit ihtiyacı) tavsiye et. Teşvik bulunamazsa bu bloğu ekleme."#;

impl CfoOrchestrator {
    pub fn new(api_key: String) -> Self {
        Self {
            gemini: GeminiClient::new(api_key),
            engine: PolarsEngine::new(),
            history: Vec::new(),
            // systemInstruction (role alanı boş bırakılmalı)
            system_instruction: Content {
                role: String::new(),
                parts: vec![Part::Text {
                    text: SYSTEM_PROMPT.to_string(),
                }],
            },
            tools: vec![Tool {
                function_declarations: get_cfo_tools(),
            }],
        }
    }

    /// API anahtarı güncelleme
    pub fn update_api_key(&mut self, api_key: String) {
        self.gemini.set_api_key(api_key);
    }

    /// Gemini istemcisine referans erişimi
    pub fn gemini(&self) -> &GeminiClient {
        &self.gemini
    }

    /// Eski geçmişi temizle, ilk mesajı ve son N mesajı tut
    fn trim_history(&mut self) {
        if self.history.len() <= MAX_HISTORY_TAIL + 1 {
            return;
        }
        let first = self.history[0].clone();
        let tail = self.history[self.history.len() - MAX_HISTORY_TAIL..].to_vec();
        self.history = std::iter::once(first).chain(tail).collect();
    }

    /// Ana ajan döngüsü (Tool kullanımı içerir, max 6 tur)
    pub async fn process_message(
        &mut self,
        user_message: &str,
    ) -> AeraResult<(String, Vec<String>)> {
        let mut tools_used: Vec<String> = Vec::new();
        let mut called_financial_tool = false;
        let mut last_incentives_result: Option<serde_json::Value> = None;
        let mut last_cashflow_result: Option<serde_json::Value> = None;

        // Kullanıcı mesajını history'e ekle
        self.history.push(Content {
            role: "user".to_string(),
            parts: vec![Part::Text {
                text: user_message.to_string(),
            }],
        });

        for round in 0..MAX_TOOL_ROUNDS {
            tracing::debug!("🔄 Agentic tur {} / {}", round + 1, MAX_TOOL_ROUNDS);

            // Token limiti için history'i buda
            self.trim_history();

            // Gemini API'ye istek
            let response = {
                let request = GeminiRequest {
                    system_instruction: Some(&self.system_instruction),
                    contents: &self.history,
                    tools: Some(self.tools.as_slice()),
                    generation_config: Some(GenerationConfig::default()),
                };
                match self.gemini.generate(&request).await {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = e.to_string();
                        let empty_stop = msg.contains("RETRYABLE_EMPTY")
                            || msg.contains("boş yanıt")
                            || msg.contains("boş content");
                        // Tool çağrıları YAPILDIYSA model özetlemeyi reddediyor demektir
                        // (özellikle flash-lite + STOP+empty). Ham sonuçlardan fallback üret.
                        if empty_stop && !tools_used.is_empty() {
                            tracing::warn!(
                                "⚠️  Gemini özetleme katmanı boş döndü, kod fallback'ı devrede"
                            );
                            let fallback = build_tool_fallback(
                                &self.engine,
                                &tools_used,
                                last_incentives_result.as_ref(),
                                last_cashflow_result.as_ref(),
                            );
                            self.history.push(Content {
                                role: "model".to_string(),
                                parts: vec![Part::Text {
                                    text: fallback.clone(),
                                }],
                            });
                            return Ok((fallback, tools_used));
                        }
                        return Err(e);
                    }
                }
            };

            // Güvenlik filtreleri veya boş yanıt kontrolü
            let candidates = match response["candidates"].as_array() {
                Some(arr) if !arr.is_empty() => arr,
                _ => {
                    // Bloklanma nedeni
                    let block_reason = response["promptFeedback"]["blockReason"]
                        .as_str()
                        .unwrap_or("UNKNOWN");
                    return Err(AeraError::GeminiApiError(format!(
                        "Gemini yanıt vermedi (blockReason: {}). Soruyu yeniden ifade edin.",
                        block_reason
                    )));
                }
            };

            let content = &candidates[0]["content"];
            let parts = match content["parts"].as_array() {
                Some(p) if !p.is_empty() => p,
                _ => {
                    let finish = candidates[0]["finishReason"].as_str().unwrap_or("?");
                    return Err(AeraError::GeminiApiError(format!(
                        "Gemini boş yanıt döndü (finishReason: {})",
                        finish
                    )));
                }
            };

            // Text ve FunctionCall ayrımı
            let mut function_calls_this_round: Vec<(String, serde_json::Value)> = Vec::new();
            let mut text_parts: Vec<String> = Vec::new();

            for part in parts {
                if let Some(fc) = part.get("functionCall") {
                    let name = fc["name"].as_str().unwrap_or("unknown").to_string();
                    let args = fc["args"].clone();
                    function_calls_this_round.push((name, args));
                } else if let Some(text) = part["text"].as_str() {
                    text_parts.push(text.to_string());
                }
            }

            if !function_calls_this_round.is_empty() {
                // FunctionCall isteklerini history'e kaydet
                let model_fc_parts: Vec<Part> = function_calls_this_round
                    .iter()
                    .map(|(name, args)| Part::FunctionCall {
                        function_call: FunctionCall {
                            name: name.clone(),
                            args: args.clone(),
                        },
                    })
                    .collect();

                self.history.push(Content {
                    role: "model".to_string(),
                    parts: model_fc_parts,
                });

                // Çağrılan araçları çalıştır
                for (tool_name, tool_args) in &function_calls_this_round {
                    tracing::info!("🔧 Araç: {}({:?})", tool_name, tool_args);
                    tools_used.push(tool_name.clone());

                    let tool_result = self.execute_tool(tool_name, tool_args);

                    let result_json = match tool_result {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({ "error": e.to_string() }),
                    };

                    if matches!(
                        tool_name.as_str(),
                        "analyze_cash_flow"
                            | "get_health_score"
                            | "predict_cashflow"
                            | "detect_cash_crunch"
                            | "detect_anomalies"
                    ) {
                        called_financial_tool = true;
                    }
                    if tool_name == "search_incentives" {
                        last_incentives_result = Some(result_json.clone());
                    }
                    if tool_name == "predict_cashflow" {
                        last_cashflow_result = Some(result_json.clone());
                    }

                    self.history.push(Content {
                        role: "function".to_string(),
                        parts: vec![Part::FunctionResponse {
                            function_response: FunctionResponse {
                                name: tool_name.clone(),
                                response: result_json,
                            },
                        }],
                    });
                }

                continue;
            }

            // Eğer function_call yoksa döngü biter, text dönülür
            let mut final_text = text_parts.join("\n").trim().to_string();
            if final_text.is_empty() {
                return Err(AeraError::GeminiApiError(
                    "Gemini ne metin ne de function_call içeren boş yanıt döndü.".into(),
                ));
            }

            // Model [AERA_METRICS] eklemeyi unutursa zorla ekle
            if !final_text.contains("[AERA_METRICS]")
                && (called_financial_tool || self.engine.has_data())
            {
                final_text.push_str(&build_metrics_block(&self.engine));
            }
            if !final_text.contains("[AERA_INCENTIVES]") {
                if let Some(ref inc) = last_incentives_result {
                    if let Some(programs) = inc.get("tesvik_programlari") {
                        final_text
                            .push_str(&format!("\n[AERA_INCENTIVES]{programs}[/AERA_INCENTIVES]",));
                    }
                }
            }
            if !final_text.contains("[AERA_CASHFLOW]") {
                if let Some(ref cf) = last_cashflow_result {
                    if cf.get("projeksiyon").is_some() {
                        let block = serde_json::json!({
                            "projeksiyon": cf["projeksiyon"],
                            "tahmin_yontemi": cf["tahmin_yontemi"],
                            "aylik_trend_gelir": cf["aylik_trend_gelir"],
                            "aylik_trend_gider": cf["aylik_trend_gider"],
                        });
                        final_text.push_str(&format!("\n[AERA_CASHFLOW]{block}[/AERA_CASHFLOW]",));
                    }
                }
            }

            // Son metin yanıtını history'e ekle
            self.history.push(Content {
                role: "model".to_string(),
                parts: vec![Part::Text {
                    text: final_text.clone(),
                }],
            });

            tracing::info!(
                "✅ Agentic döngü tamamlandı ({} tur, {} araç)",
                round + 1,
                tools_used.len()
            );

            return Ok((final_text, tools_used));
        }

        Err(AeraError::AgentExecutionError(format!(
            "Agentic döngü {} turda tamamlanamadı. Lütfen soruyu basitleştirin.",
            MAX_TOOL_ROUNDS
        )))
    }

    /// Araç çalıştırıcı
    fn execute_tool(
        &mut self,
        name: &str,
        args: &serde_json::Value,
    ) -> AeraResult<serde_json::Value> {
        match name {
            "analyze_cash_flow" => {
                let income = args["income_column"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| self.engine.income_col.clone());
                let expense = args["expense_column"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| self.engine.expense_col.clone());
                self.engine.analyze_cash_flow(&income, &expense)
            }

            "get_data_summary" => {
                if !self.engine.has_data() {
                    return Ok(serde_json::json!({
                        "data_loaded": false,
                        "message": "Veri yüklenmemiş. Kullanıcıya CSV yüklemesini önerin."
                    }));
                }
                let summary = self.engine.summary()?;
                let columns = self.engine.get_columns()?;
                Ok(serde_json::json!({
                    "data_loaded": true,
                    "summary": summary,
                    "columns": columns
                }))
            }

            "search_incentives" => {
                let query = args["query"].as_str().unwrap_or("KOBİ destek");
                Ok(crate::infrastructure::incentives_db::search_incentives(
                    query,
                ))
            }

            "predict_cashflow" => {
                // Ay sınırı 1-6
                let months = args["months_ahead"].as_u64().unwrap_or(3) as usize;
                let months = months.clamp(1, 6);
                Ok(self.engine.predict_cashflow(months))
            }

            // F-04: Finansal Sağlık Skoru
            "get_health_score" => Ok(self.engine.health_score()),

            // F-06: Türkiye Sektör Benchmark
            "compare_sector_benchmark" => {
                let sektor = args["sektor"].as_str().unwrap_or("genel");
                Ok(sector_benchmark(sektor, &self.engine))
            }

            // F-07: Kategori bazlı gider analizi
            "analyze_expense_categories" => {
                let cats = self.engine.expense_by_category();
                if cats.is_empty() {
                    Ok(serde_json::json!({
                        "mesaj": "Kategori verisi bulunamadı. CSV'de 'kategori' sütunu olduğundan emin olun.",
                        "kategoriler": []
                    }))
                } else {
                    Ok(serde_json::json!({
                        "toplam_kategori": cats.len(),
                        "kategoriler": cats
                    }))
                }
            }

            // F-NEW: Z-score anomali tespiti
            "detect_anomalies" => Ok(self.engine.detect_anomalies()),

            // F-NEW: What-If senaryo simülasyonu
            "simulate_scenario" => {
                // Gelen NaN/Inf değerlerini filtrele
                let raw_inc = args["extra_monthly_income"].as_f64().unwrap_or(0.0);
                let raw_exp = args["extra_monthly_expense"].as_f64().unwrap_or(0.0);
                let extra_income = if raw_inc.is_finite() { raw_inc } else { 0.0 };
                let extra_expense = if raw_exp.is_finite() { raw_exp } else { 0.0 };
                let months = (args["months"].as_u64().unwrap_or(3) as usize).clamp(1, 6);
                Ok(self
                    .engine
                    .simulate_scenario(extra_income, extra_expense, months))
            }

            // F-NEW: Nakit sıkışıklığı tespiti
            "detect_cash_crunch" => Ok(self.engine.detect_cash_crunch()),

            unknown => Ok(serde_json::json!({
                "error": format!("Bilinmeyen araç: '{}'. Tanımlı araçlar: analyze_cash_flow, get_data_summary, search_incentives, predict_cashflow, get_health_score, compare_sector_benchmark, analyze_expense_categories, detect_anomalies, simulate_scenario, detect_cash_crunch", unknown)
            })),
        }
    }

    /// CSV verisini yükler
    pub fn load_data_from_string(
        &mut self,
        csv_data: &str,
        income: Option<String>,
        expense: Option<String>,
        date: Option<String>,
    ) -> AeraResult<(usize, usize, Vec<String>)> {
        self.engine.configure_columns(income, expense, date);
        self.engine.load_csv_from_string(csv_data)
    }

    /// Geçmiş mesaj sayısı
    #[cfg(test)]
    fn history_len(&self) -> usize {
        self.history.len()
    }
}

// Testlerin yeri (semantic sıralama korundu)
#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    const TEST_CSV: &str = "tarih,gelir,gider\n\
        2026-01-01,100000,60000\n\
        2026-02-01,110000,65000\n\
        2026-03-01,105000,62000";

    fn orc_with_data() -> CfoOrchestrator {
        let mut o = CfoOrchestrator::new("test_key".to_string());
        o.load_data_from_string(TEST_CSV, None, None, None)
            .expect("CSV yüklenemedi");
        o
    }

    #[test]
    fn test_tool_health_score() {
        let mut o = orc_with_data();
        let r = o
            .execute_tool("get_health_score", &serde_json::json!({}))
            .unwrap();
        assert!(r["skor"].is_number(), "skor sayısal olmalı");
        assert!(r["harf"].is_string(), "harf string olmalı");
    }

    #[test]
    fn test_tool_analyze_cash_flow() {
        let mut o = orc_with_data();
        let r = o
            .execute_tool("analyze_cash_flow", &serde_json::json!({}))
            .unwrap();
        assert!(r["burn_rate"].is_string(), "burn_rate string olmalı");
        assert!(
            r["risk_seviyesi"].is_string(),
            "risk_seviyesi string olmalı"
        );
    }

    #[test]
    fn test_tool_predict_cashflow() {
        let mut o = orc_with_data();
        let r = o
            .execute_tool("predict_cashflow", &serde_json::json!({"months_ahead": 3}))
            .unwrap();
        let proj = r["projeksiyon"]
            .as_array()
            .expect("projeksiyon dizisi olmalı");
        assert_eq!(proj.len(), 3, "3 aylık projeksiyon bekleniyor");
        assert!(proj[0]["ci_alt"].is_number(), "CI alanı olmalı");
    }

    #[test]
    fn test_tool_detect_anomalies() {
        let mut o = orc_with_data();
        let r = o
            .execute_tool("detect_anomalies", &serde_json::json!({}))
            .unwrap();
        assert!(
            r["anomali_sayisi"].is_number(),
            "anomali_sayisi sayısal olmalı"
        );
    }

    #[test]
    fn test_tool_detect_cash_crunch() {
        let mut o = orc_with_data();
        let r = o
            .execute_tool("detect_cash_crunch", &serde_json::json!({}))
            .unwrap();
        assert!(r["risk"].is_string(), "risk string olmalı");
    }

    #[test]
    fn test_tool_simulate_scenario() {
        let mut o = orc_with_data();
        let r = o
            .execute_tool(
                "simulate_scenario",
                &serde_json::json!({
                    "extra_monthly_income":  10000.0,
                    "extra_monthly_expense": 0.0,
                    "months":                3
                }),
            )
            .unwrap();
        let sim_net = r["simule"]["aylik_net"]
            .as_f64()
            .expect("simule.aylik_net sayısal olmalı");
        assert!(
            sim_net > 0.0,
            "Ek 10K gelir pozitif net üretmeli: {}",
            sim_net
        );
    }

    #[test]
    fn test_tool_search_incentives_returns_results() {
        let mut o = CfoOrchestrator::new("test".to_string());
        let r = o
            .execute_tool(
                "search_incentives",
                &serde_json::json!({"query": "teknoloji"}),
            )
            .unwrap();
        assert!(
            r["sonuc_sayisi"].as_u64().unwrap_or(0) > 0,
            "Teknoloji sorgusu sonuç dönmeli"
        );
    }

    #[test]
    fn test_tool_get_data_summary_no_data() {
        let mut o = CfoOrchestrator::new("test".to_string());
        let r = o
            .execute_tool("get_data_summary", &serde_json::json!({}))
            .unwrap();
        assert_eq!(r["data_loaded"], false, "Veri yüklenmemişken false olmalı");
    }

    #[test]
    fn test_tool_unknown_returns_error_field() {
        let mut o = CfoOrchestrator::new("test".to_string());
        let r = o
            .execute_tool("var_olmayan_arac", &serde_json::json!({}))
            .unwrap();
        assert!(
            r["error"].is_string(),
            "Bilinmeyen araç error alanı döndürmeli"
        );
    }

    #[test]
    fn test_trim_history_respects_max_tail() {
        let mut o = CfoOrchestrator::new("test".to_string());
        // 30 mesaj ekle
        for i in 0..30_usize {
            o.history.push(Content {
                role: "user".to_string(),
                parts: vec![Part::Text {
                    text: format!("mesaj {}", i),
                }],
            });
        }
        o.trim_history();
        assert!(
            o.history_len() <= 21,
            "trim_history sonrası max 21 mesaj: {}",
            o.history_len()
        );
    }

    #[test]
    fn test_trim_history_keeps_first_message() {
        let mut o = CfoOrchestrator::new("test".to_string());
        for i in 0..25_usize {
            o.history.push(Content {
                role: "user".to_string(),
                parts: vec![Part::Text {
                    text: format!("mesaj {}", i),
                }],
            });
        }
        o.trim_history();
        // İlk mesaj korunmalı
        if let Some(Part::Text { text }) = o.history.first().and_then(|c| c.parts.first()) {
            assert_eq!(text, "mesaj 0", "İlk mesaj korunmalı");
        }
    }
}

// UI için özet metrik bloğu oluştur
fn build_metrics_block(engine: &crate::infrastructure::polars_engine::PolarsEngine) -> String {
    if !engine.has_data() {
        return String::new();
    }
    let inc_col = engine.income_col.clone();
    let exp_col = engine.expense_col.clone();

    let (net, burn, runway, risk) = match (engine.column_sum(&inc_col), engine.column_sum(&exp_col))
    {
        (Ok(total_inc), Ok(total_exp)) => {
            let net = total_inc - total_exp;
            let months = engine
                .date_range
                .as_ref()
                .map(|d| (d.days as f64 / 30.44).max(1.0))
                .unwrap_or(1.0);
            let monthly_exp = total_exp / months;
            let monthly_inc = total_inc / months;
            let burn = monthly_exp;
            let runway = if monthly_exp > monthly_inc && monthly_exp > 0.0 {
                (net.abs() / (monthly_exp - monthly_inc)).min(999.0)
            } else {
                999.0
            };
            let risk = if runway < 1.0 {
                "KRİTİK"
            } else if runway < 3.0 {
                "YÜKSEK"
            } else if runway < 6.0 {
                "ORTA"
            } else {
                "DÜŞÜK"
            };
            (net, burn, runway, risk)
        }
        _ => (0.0, 0.0, 999.0, "ORTA"),
    };

    let health = engine.health_score();
    let skor = health["skor"].as_i64().unwrap_or(0);
    let harf = health["harf"].as_str().unwrap_or("C");
    let emoji = health["emoji"].as_str().unwrap_or("🟡");

    format!(
        "\n[AERA_METRICS]{{\"net_nakit\":{net:.2},\"risk\":\"{risk}\",\"burn_rate\":{burn:.2},\"runway_ay\":{runway:.2},\"health_skor\":{skor},\"health_harf\":\"{harf}\",\"health_emoji\":\"{emoji}\"}}[/AERA_METRICS]"
    )
}

// Gemini boş döndüğünde tool sonuçlarından özet üret — kullanıcıyı hata mesajıyla terk etme
fn build_tool_fallback(
    engine: &crate::infrastructure::polars_engine::PolarsEngine,
    tools_used: &[String],
    incentives: Option<&serde_json::Value>,
    cashflow: Option<&serde_json::Value>,
) -> String {
    let mut out = String::new();

    if !engine.has_data() {
        out.push_str("**Veri yüklenmemiş** — analiz için önce CSV yükleyin veya sol panelden bir demo sektörü seçin.\n\n");
        out.push_str("Sorunuzu yanıtlayabilmem için aylık gelir/gider verilerine ihtiyacım var. Demo seçtikten sonra aynı soruyu tekrar sorun.");
        return out;
    }

    out.push_str("**Aera özet** (kullanılan araçlar: ");
    out.push_str(&tools_used.join(", "));
    out.push_str(")\n\n");

    // Cashflow tahmini varsa kısa özet
    if let Some(cf) = cashflow {
        if let Some(proj) = cf.get("projeksiyon").and_then(|v| v.as_array()) {
            out.push_str("**Nakit projeksiyonu:**\n");
            for (i, m) in proj.iter().take(3).enumerate() {
                let net = m.get("tahmini_net").and_then(|v| v.as_f64()).unwrap_or(0.0);
                out.push_str(&format!("- Ay {}: {:.2} TL\n", i + 1, net));
            }
            out.push('\n');
        }
    }

    // Teşvik varsa kısa liste
    if let Some(inc) = incentives {
        if let Some(progs) = inc.get("tesvik_programlari").and_then(|v| v.as_array()) {
            if !progs.is_empty() {
                out.push_str("**Uygun teşvik programları:**\n");
                for p in progs.iter().take(3) {
                    let isim = p.get("isim").and_then(|v| v.as_str()).unwrap_or("?");
                    out.push_str(&format!("- {}\n", isim));
                }
                out.push('\n');
            }
        }
    }

    // Metrics bloğu ekle — UI dashboard çizecek
    out.push_str(&build_metrics_block(engine));

    if let Some(cf) = cashflow {
        if cf.get("projeksiyon").is_some() {
            let block = serde_json::json!({
                "projeksiyon": cf["projeksiyon"],
                "tahmin_yontemi": cf["tahmin_yontemi"],
                "aylik_trend_gelir": cf["aylik_trend_gelir"],
                "aylik_trend_gider": cf["aylik_trend_gider"],
            });
            out.push_str(&format!("\n[AERA_CASHFLOW]{block}[/AERA_CASHFLOW]"));
        }
    }
    if let Some(inc) = incentives {
        if let Some(progs) = inc.get("tesvik_programlari") {
            out.push_str(&format!("\n[AERA_INCENTIVES]{progs}[/AERA_INCENTIVES]"));
        }
    }

    out
}

// Sektör karşılaştırması (Benchmark)
fn sector_benchmark(
    sector: &str,
    engine: &crate::infrastructure::polars_engine::PolarsEngine,
) -> serde_json::Value {
    use serde_json::json;
    let s = sector.to_lowercase();
    let bm = if s.contains("teknoloji")
        || s.contains("yazilim")
        || s.contains("yazılım")
        || s.contains("it")
        || s.contains("bilişim")
    {
        json!({"sektor":"Teknoloji/Yazılım","ort_runway_ay":4.2,"ort_gelir_gider_orani":1.38,"ort_burn_buyume":"-2%/ay"})
    } else if s.contains("perakende")
        || s.contains("magaza")
        || s.contains("mağaza")
        || s.contains("ticaret")
    {
        json!({"sektor":"Perakende","ort_runway_ay":2.1,"ort_gelir_gider_orani":1.18,"ort_burn_buyume":"+3%/ay"})
    } else if s.contains("uretim")
        || s.contains("üretim")
        || s.contains("imalat")
        || s.contains("fabrika")
    {
        json!({"sektor":"Üretim/İmalat","ort_runway_ay":5.1,"ort_gelir_gider_orani":1.45,"ort_burn_buyume":"+1%/ay"})
    } else if s.contains("hizmet")
        || s.contains("danismanlik")
        || s.contains("danışmanlık")
        || s.contains("konsultanlik")
    {
        json!({"sektor":"Hizmet/Danışmanlık","ort_runway_ay":3.8,"ort_gelir_gider_orani":1.55,"ort_burn_buyume":"-1%/ay"})
    } else if s.contains("insaat")
        || s.contains("inşaat")
        || s.contains("gayrimenkul")
        || s.contains("emlak")
    {
        json!({"sektor":"İnşaat/Gayrimenkul","ort_runway_ay":3.2,"ort_gelir_gider_orani":1.22,"ort_burn_buyume":"+5%/ay"})
    } else if s.contains("gida")
        || s.contains("gıda")
        || s.contains("restoran")
        || s.contains("cafe")
        || s.contains("yemek")
    {
        json!({"sektor":"Gıda/Restoran","ort_runway_ay":1.8,"ort_gelir_gider_orani":1.12,"ort_burn_buyume":"+4%/ay"})
    } else if s.contains("saglik")
        || s.contains("sağlık")
        || s.contains("klinik")
        || s.contains("hastane")
    {
        json!({"sektor":"Sağlık","ort_runway_ay":4.5,"ort_gelir_gider_orani":1.42,"ort_burn_buyume":"+2%/ay"})
    } else if s.contains("egitim")
        || s.contains("eğitim")
        || s.contains("kurs")
        || s.contains("okul")
    {
        json!({"sektor":"Eğitim","ort_runway_ay":3.6,"ort_gelir_gider_orani":1.35,"ort_burn_buyume":"+1%/ay"})
    } else {
        json!({"sektor":"Genel KOBİ","ort_runway_ay":3.1,"ort_gelir_gider_orani":1.28,"ort_burn_buyume":"+2%/ay"})
    };

    let firma_metrikleri = if engine.has_data() {
        let inc_col = engine.income_col.clone();
        let exp_col = engine.expense_col.clone();
        if let (Ok(inc), Ok(exp)) = (engine.column_sum(&inc_col), engine.column_sum(&exp_col)) {
            let months = engine
                .date_range
                .as_ref()
                .map(|d| d.days as f64 / 30.44)
                .unwrap_or(1.0)
                .max(0.01);
            let monthly_inc = inc / months;
            let monthly_exp = exp / months;
            let runway = if monthly_exp > monthly_inc {
                ((inc - exp) / (monthly_exp - monthly_inc) * 10.0).round() / 10.0
            } else {
                24.0
            };
            let ratio = if exp > 0.0 {
                (inc / exp * 100.0).round() / 100.0
            } else {
                2.0
            };
            json!({"runway_ay": runway, "gelir_gider_orani": ratio})
        } else {
            json!(null)
        }
    } else {
        json!(null)
    };

    let sekt_runway = bm["ort_runway_ay"].as_f64().unwrap_or(3.1);
    let sekt_ratio = bm["ort_gelir_gider_orani"].as_f64().unwrap_or(1.28);
    let sektor_adi = bm["sektor"].as_str().unwrap_or("KOBİ");

    let karsilastirma = if let Some(fm) = firma_metrikleri.as_object() {
        let firma_runway = fm.get("runway_ay").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let firma_ratio = fm
            .get("gelir_gider_orani")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let runway_diff = firma_runway - sekt_runway;
        let ratio_diff = firma_ratio - sekt_ratio;
        format!(
            "Runway: firmanız {:.1} ay ({} sektörü ort. {:.1} ay) — {}. Gelir/gider oranı: firmanız {:.2} (sektör ort. {:.2}) — {}.",
            firma_runway, sektor_adi, sekt_runway,
            if runway_diff >= 0.0 { "sektör ÜZERİNDE 👍" } else { "sektörün ALTINDA ⚠️" },
            firma_ratio, sekt_ratio,
            if ratio_diff >= 0.0 { "sektör ÜZERİNDE 👍" } else { "sektörün ALTINDA ⚠️" },
        )
    } else {
        format!(
            "{} sektörü ortalaması — runway: {:.1} ay, gelir/gider: {:.2}",
            sektor_adi, sekt_runway, sekt_ratio
        )
    };

    json!({
        "benchmark": bm,
        "firma_metrikleri": firma_metrikleri,
        "karsilastirma_mesaji": karsilastirma,
        "tavsiye": format!("{} sektöründeki firmalar için KOSGEB ve TÜBİTAK destek programlarını inceleyin.", sektor_adi),
        "kaynak": "Tahmini sektör ortalamaları"
    })
}
