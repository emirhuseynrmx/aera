use axum::{
    body::Bytes,
    extract::{ConnectInfo, State},
    Json,
    response::{IntoResponse, Response},
    http::header,
};
use std::net::SocketAddr;
use std::time::Instant;
use uuid::Uuid;
use crate::api::schemas::{ChatRequest, ChatResponse, HealthResponse, UploadResponse};
use crate::api::state::{AppState, short_id};
use crate::core::error::AeraError;

// session_id validasyonu (path traversal / disk limit koruması).
const MAX_SESSION_ID_LEN: usize = 64;
// Max prompt uzunluğu (Token kotası koruması).
const MAX_CHAT_MESSAGE_LEN: usize = 8192;

/// session_id format validasyonu ve default UUID ataması.
fn normalize_session_id(raw: Option<String>) -> Result<String, AeraError> {
    let sid = raw.unwrap_or_else(|| Uuid::new_v4().to_string());
    if sid.is_empty() {
        return Ok(Uuid::new_v4().to_string());
    }
    if sid.len() > MAX_SESSION_ID_LEN {
        return Err(AeraError::BadRequest(format!(
            "session_id en fazla {} karakter olabilir.", MAX_SESSION_ID_LEN
        )));
    }
    // Alphanumeric + dash + underscore
    if !sid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(AeraError::BadRequest(
            "session_id sadece harf, rakam, '-' ve '_' içerebilir.".into()
        ));
    }
    Ok(sid)
}

/// X-API-Key header'ı veya ENV üzerinden anahtarı çözer.
fn resolve_api_key(
    headers: &header::HeaderMap,
    fallback: &str,
) -> Result<Option<String>, AeraError> {
    if let Some(key) = headers.get("x-api-key").and_then(|h| h.to_str().ok()).filter(|s| !s.is_empty()) {
        return Ok(Some(key.to_string()));
    }
    if !fallback.is_empty() {
        // Fallback: sunucu ENV key'ini kullan.
        tracing::warn!("⚠️ X-API-Key gönderilmedi — server env key kullanılıyor.");
        return Ok(None);
    }
    Err(AeraError::Unauthorized(
        "API anahtarı gerekli. X-API-Key header'ı ile gönderin.".into()
    ))
}

/// Proxy arkasındaki gerçek istemci IP'sini bulur (TRUST_PROXY_HEADERS flag'i varsa).
fn client_ip(headers: &header::HeaderMap, connect_info: SocketAddr) -> String {
    let trust_proxy = std::env::var("TRUST_PROXY_HEADERS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if trust_proxy {
        if let Some(v) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
            if !v.is_empty() { return v.to_string(); }
        }
        if let Some(v) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
            if let Some(first) = v.split(',').next() {
                let trimmed = first.trim();
                if !trimmed.is_empty() { return trimmed.to_string(); }
            }
        }
    }
    connect_info.ip().to_string()
}

/// GET /health
pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    Json(HealthResponse {
        status: "operational",
        version: env!("CARGO_PKG_VERSION"),
        engine: "Rust/Axum + Polars + Gemini 2.0 Flash",
        active_sessions: state.session_count(),
    })
}

/// POST /api/chat
pub async fn chat_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: header::HeaderMap,
    Json(payload): Json<ChatRequest>,
) -> Result<impl IntoResponse, AeraError> {
    let start = Instant::now();

    // Mesaj boyutu sınırı — body limit'i 10MB ama tek bir prompt 8KB'ı geçmemeli
    if payload.message.len() > MAX_CHAT_MESSAGE_LEN {
        return Err(AeraError::BadRequest(format!(
            "Mesaj çok uzun ({} byte). Sınır: {} byte.",
            payload.message.len(), MAX_CHAT_MESSAGE_LEN
        )));
    }
    if payload.message.trim().is_empty() {
        return Err(AeraError::BadRequest("Boş mesaj gönderilemez.".into()));
    }

    let session_id = normalize_session_id(payload.session_id)?;
    let _ip = client_ip(&headers, addr);
    let user_key = resolve_api_key(&headers, &state.gemini_api_key)?;

    tracing::info!("💬 [{}] Mesaj: {:.60}...", short_id(&session_id), payload.message);

    let orchestrator_arc = state.get_or_create_session(&session_id)?;
    let mut orchestrator = orchestrator_arc.lock().await;

    if let Some(key) = user_key {
        orchestrator.update_api_key(key);
    }

    // Agent pipeline'ını tetikle
    let coord_response = crate::agents::coordinator::run(
        &mut orchestrator,
        &payload.message,
    ).await?;

    let latency_ms = start.elapsed().as_millis() as u64;
    tracing::info!(
        "✅ [{}] {}ms | plan:{}sub | exec:{}tool | critic:{}",
        short_id(&session_id),
        latency_ms,
        coord_response.trace.subtask_count,
        coord_response.tools_used.len(),
        coord_response.trace.critic_verdict,
    );

    Ok(Json(ChatResponse {
        reply: coord_response.reply,
        tools_used: coord_response.tools_used,
        latency_ms,
        session_id,
        agent_trace: coord_response.trace,
    }))
}

/// GET /api/demo — İki mod:
/// 1. **Anchor:** `?scenario=X` → data/ klasöründen statik CSV (build-deterministik)
/// 2. **Live:** `?generate=true&sector=X&pattern=Y&months=N` → data_generator ile her seferinde farklı seed'le veri üretir.
///
/// Diğer param: `session_id` (opsiyonel).
pub async fn demo_handler(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, AeraError> {
    const FALLBACK_CSV: &str = include_str!("../../data/demo_kobi.csv");

    let session_id = normalize_session_id(params.get("session_id").cloned())?;

    let want_generate = params.get("generate")
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let (csv_data, scenario_name) = if want_generate {
        // Canlı üretim (dinamik veri üretir)
        generate_live_demo(&params)?
    } else {
        // Statik dosya oku
        let scenario = params.get("scenario").and_then(|s| sanitize_scenario(s));
        load_demo_csv(scenario, FALLBACK_CSV)
    };

    let user_key = resolve_api_key(&headers, &state.gemini_api_key)?;
    let orchestrator_arc = state.get_or_create_session(&session_id)?;
    let mut orchestrator = orchestrator_arc.lock().await;

    if let Some(key) = user_key {
        orchestrator.update_api_key(key);
    }

    let (rows, cols, col_names) = orchestrator
        .load_data_from_string(&csv_data, None, None, None)?;

    let date_range = orchestrator.engine.date_range.clone();
    let monthly_data = orchestrator.engine.monthly_breakdown();

    tracing::info!("🎯 Demo [{}] yüklendi: {}", scenario_name, short_id(&session_id));

    Ok(Json(UploadResponse {
        success: true,
        message: format!("{} verisi yüklendi: {} işlem, {} ay.", scenario_name, rows, monthly_data.len()),
        rows,
        columns: cols,
        column_names: col_names,
        date_range,
        monthly_data,
    }))
}

/// Query bazlı live demo (rastgele seed ile).
fn generate_live_demo(
    params: &std::collections::HashMap<String, String>,
) -> Result<(String, String), AeraError> {
    use crate::infrastructure::data_generator::{find_profile, generate, Pattern, all_ids};

    let sector = params.get("sector").and_then(|s| sanitize_scenario(s));
    let profile = match sector.as_deref().and_then(find_profile) {
        Some(p) => p,
        None => {
            // Geçersiz sektör -> 400
            return Err(AeraError::BadRequest(format!(
                "Geçerli 'sector' parametresi gerekli. Desteklenen: {}",
                all_ids().join(", ")
            )));
        }
    };

    let pattern = params.get("pattern")
        .map(|s| Pattern::parse(s))
        .unwrap_or(Pattern::Stable);

    // Ay limitleri (6-24)
    let months = params.get("months")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(15)
        .clamp(6, 24);

    // Rastgele seed
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xDEADBEEF);

    // Başlangıç tarihi hesabı (son veri = bu ay)
    use chrono::Datelike;
    let now = chrono::Local::now().date_naive();
    let start_year = now.year();
    let start_month_zero_based = now.month0() as i32 - months as i32 + 1;
    let (sy, sm0) = if start_month_zero_based < 0 {
        let years_back = (-start_month_zero_based + 11) / 12;
        (start_year - years_back, (start_month_zero_based + years_back * 12) as u32)
    } else {
        (start_year, start_month_zero_based as u32)
    };
    let start = chrono::NaiveDate::from_ymd_opt(sy, sm0 + 1, 1)
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());

    let csv = generate(profile, pattern, months, start, seed);
    let label = format!("{} (live, {} ay)", profile.display, months);
    Ok((csv, label))
}

/// CSV path traversal koruması. (Sadece a-z0-9_ ve max 32 char)
fn sanitize_scenario(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > 32 { return None; }
    if !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return None;
    }
    Some(s.to_string())
}

fn load_demo_csv(scenario: Option<String>, fallback: &'static str) -> (String, String) {
    let data_dir = std::path::Path::new("data");

    if let Some(ref s) = scenario {
        let path = data_dir.join(format!("demo_{}.csv", s));
        if let Ok(content) = std::fs::read_to_string(&path) {
            let name = s.replace('_', " ").to_uppercase();
            return (content, name);
        }
    }

    // Rastgele bir demo CSV seç
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        let demos: Vec<_> = entries
            .flatten()
            .filter(|e| {
                let name = e.file_name();
                let n = name.to_string_lossy();
                n.starts_with("demo_") && n.ends_with(".csv")
            })
            .collect();

        if !demos.is_empty() {
            // Basit pseudo-random: zamanı kullan
            let idx = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as usize)
                .unwrap_or(0) % demos.len();

            if let Ok(content) = std::fs::read_to_string(demos[idx].path()) {
                let raw = demos[idx].file_name();
                let name = raw.to_string_lossy()
                    .trim_start_matches("demo_")
                    .trim_end_matches(".csv")
                    .replace('_', " ")
                    .to_uppercase();
                return (content, name);
            }
        }
    }

    (fallback.to_string(), "Demo KOBİ".to_string())
}

/// POST /api/export/pdf
pub async fn export_pdf_handler(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Response, AeraError> {
    let session_id_raw = params.get("session_id").cloned().unwrap_or_default();
    if session_id_raw.is_empty() {
        return Err(AeraError::BadRequest("session_id zorunlu".into()));
    }
    // Güvenlik (path traversal engelleme)
    let session_id = normalize_session_id(Some(session_id_raw))?;

    let user_key = resolve_api_key(&headers, &state.gemini_api_key)?;
    let orchestrator_arc = state.get_or_create_session(&session_id)?;
    let mut orchestrator = orchestrator_arc.lock().await;

    if let Some(key) = user_key {
        orchestrator.update_api_key(key);
    }

    if !orchestrator.engine.has_data() {
        return Err(AeraError::BadRequest(
            "PDF oluşturmak için önce veri yükleyin.".into()
        ));
    }

    let pdf_bytes = generate_pdf_report(&orchestrator.engine, &session_id).await?;

    let is_download = params.get("download").map(|s| s == "1").unwrap_or(false);
    let disposition = if is_download {
        "attachment; filename=\"AeraCFO_Finansal_Rapor.pdf\""
    } else {
        "inline; filename=\"AeraCFO_Finansal_Rapor.pdf\""
    };

    let response = axum::response::Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(axum::body::Body::from(pdf_bytes))
        .map_err(|e| AeraError::AgentExecutionError(e.to_string()))?;

    Ok(response)
}

async fn generate_pdf_report(engine: &crate::infrastructure::polars_engine::PolarsEngine, session_id: &str) -> Result<Vec<u8>, AeraError> {
    let health = engine.health_score();
    let skor   = health["skor"].as_i64().unwrap_or(0);
    let harf   = health["harf"].as_str().unwrap_or("C");

    let inc_col = engine.income_col.clone();
    let exp_col = engine.expense_col.clone();
    let (total_gelir, total_gider, net) = if let (Ok(g), Ok(e)) = (engine.column_sum(&inc_col), engine.column_sum(&exp_col)) {
        (g, e, g - e)
    } else {
        (0.0, 0.0, 0.0)
    };

    let months = engine.date_range.as_ref()
        .map(|d| (d.days as f64 / 30.44).max(1.0))
        .unwrap_or(1.0);
    let monthly_gelir = total_gelir / months;
    let monthly_gider = total_gider / months;
    let runway_val = if monthly_gider > monthly_gelir && monthly_gider > 0.0 {
        net.abs() / (monthly_gider - monthly_gelir)
    } else { 999.0 };
    let runway_str = if runway_val >= 999.0 { "Pozitif Akış".to_string() } else { format!("{:.1} ay", runway_val) };

    let risk_label = if runway_val < 1.0 { "KRİTİK" } else if runway_val < 3.0 { "YÜKSEK" } else if runway_val < 6.0 { "ORTA" } else { "DÜŞÜK" };
    let risk_color = if runway_val < 1.0 { "#ef4444" } else if runway_val < 3.0 { "#f97316" } else if runway_val < 6.0 { "#eab308" } else { "#22c55e" };
    let skor_color = if skor >= 80 { "#22c55e" } else if skor >= 60 { "#eab308" } else if skor >= 40 { "#f97316" } else { "#ef4444" };

    let date_range_str = engine.date_range.as_ref()
        .map(|d| format!("{} / {}", d.start, d.end))
        .unwrap_or_else(|| "Bilinmiyor".to_string());

    let monthly_data = engine.monthly_breakdown();
    let now = chrono::Local::now().format("%d.%m.%Y %H:%M").to_string();

    // Trend karşılaştırması
    let trend = if monthly_data.len() >= 2 {
        let first_net = monthly_data[0]["gelir"].as_f64().unwrap_or(0.0) - monthly_data[0]["gider"].as_f64().unwrap_or(0.0);
        let last_net = monthly_data[monthly_data.len()-1]["gelir"].as_f64().unwrap_or(0.0) - monthly_data[monthly_data.len()-1]["gider"].as_f64().unwrap_or(0.0);
        if last_net > first_net { "Yükseliş Trendi" } else if last_net < first_net { "Düşüş Trendi" } else { "Yatay Trend" }
    } else { "Veri Yetersiz" };

    let t = build_typst_report(
        &date_range_str, &now,
        net, total_gelir, total_gider, monthly_gelir, monthly_gider,
        skor, harf, &runway_str, risk_label, risk_color, skor_color, trend,
        &monthly_data,
    );

    let tmp_dir = std::env::temp_dir();
    let safe_sid = session_id.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "");
    // Concurrency için unique dosya ismi
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let typ_path = tmp_dir.join(format!("aeracfo_{safe_sid}_{nonce}.typ"));
    let pdf_path = tmp_dir.join(format!("aeracfo_{safe_sid}_{nonce}.pdf"));

    let typ_str = typ_path.to_str()
        .ok_or_else(|| AeraError::AgentExecutionError("Temp path UTF-8 değil".into()))?;
    let pdf_str = pdf_path.to_str()
        .ok_or_else(|| AeraError::AgentExecutionError("Temp path UTF-8 değil".into()))?;

    tokio::fs::write(&typ_path, &t)
        .await
        .map_err(|e| AeraError::AgentExecutionError(format!("Typst dosyasi yazilamadi: {e}")))?;

    let typst_bin = std::env::var("TYPST_BIN").unwrap_or_else(|_| "typst".to_string());

    let output = tokio::process::Command::new(&typst_bin)
        .args(["compile", typ_str, pdf_str])
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .output()
        .await;

    // Disk şişmemesi için temp dosya temizliği
    let cleanup = |paths: &[std::path::PathBuf]| {
        for p in paths {
            let _ = std::fs::remove_file(p);
        }
    };

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            cleanup(&[typ_path.clone(), pdf_path.clone()]);
            return Err(AeraError::AgentExecutionError(format!(
                "Typst calistirilamadi ({}): {e}. TYPST_BIN env değişkeniyle path verin.",
                typst_bin
            )));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        cleanup(&[typ_path.clone(), pdf_path.clone()]);
        return Err(AeraError::AgentExecutionError(format!("Typst hatasi: {stderr}")));
    }

    let pdf_bytes = tokio::fs::read(&pdf_path).await;
    cleanup(&[typ_path, pdf_path]);
    pdf_bytes.map_err(|e| AeraError::AgentExecutionError(format!("PDF okunamadı: {e}")))
}

// Typst raporu — template templates/report.typ dosyasında, sadece değişken bağlama burada.
// Eskiden 150+ satır push_str ile inline string vardı; bakım kâbusuydu.
const TYPST_TEMPLATE: &str = include_str!("../../templates/report.typ");

#[allow(clippy::too_many_arguments)]
fn build_typst_report(
    date_range: &str, now: &str,
    net: f64, total_gelir: f64, total_gider: f64,
    monthly_gelir: f64, monthly_gider: f64,
    skor: i64, harf: &str,
    runway: &str, risk_label: &str, risk_color: &str, skor_color: &str,
    trend: &str,
    monthly_data: &[serde_json::Value],
) -> String {
    let net_sign = if net >= 0.0 { "+" } else { "" };
    let net_color = if net >= 0.0 { "#16a34a" } else { "#dc2626" };

    // KPI kartları (4 sütun)
    let kpi_health_label = if skor >= 80 { "Sağlıklı" }
        else if skor >= 60 { "Dikkat" }
        else if skor >= 40 { "Riskli" }
        else { "Kritik" };
    let kpi_cards = format!(
        "  block(fill: rgb(\"#1e293b\"), stroke: 0.5pt + rgb(\"#334155\"), radius: 4pt, inset: (x:10pt,y:9pt))[\n    #text(size: 7pt, fill: rgb(\"#94a3b8\"))[Net Nakit Akışı]\n    #linebreak()\n    #text(size: 13pt, weight: \"bold\", fill: rgb(\"{net_color}\"))[{net_sign}{net:.0} TL]\n    #linebreak()\n    #text(size: 7pt, fill: rgb(\"#64748b\"))[Analiz dönemi toplamı]\n  ],\n\
         block(fill: rgb(\"#1e293b\"), stroke: 0.5pt + rgb(\"#334155\"), radius: 4pt, inset: (x:10pt,y:9pt))[\n    #text(size: 7pt, fill: rgb(\"#94a3b8\"))[Finansal Sağlık]\n    #linebreak()\n    #text(size: 13pt, weight: \"bold\", fill: rgb(\"{skor_color}\"))[{skor}/100]\n    #linebreak()\n    #text(size: 7pt, fill: rgb(\"#64748b\"))[Not: {harf} | {kpi_health_label}]\n  ],\n\
         block(fill: rgb(\"#1e293b\"), stroke: 0.5pt + rgb(\"#334155\"), radius: 4pt, inset: (x:10pt,y:9pt))[\n    #text(size: 7pt, fill: rgb(\"#94a3b8\"))[Aylık Burn Rate]\n    #linebreak()\n    #text(size: 13pt, weight: \"bold\", fill: rgb(\"#f97316\"))[{monthly_gider:.0} TL]\n    #linebreak()\n    #text(size: 7pt, fill: rgb(\"#64748b\"))[Ortalama aylık gider]\n  ],\n\
         block(fill: rgb(\"#1e293b\"), stroke: 0.5pt + rgb(\"#334155\"), radius: 4pt, inset: (x:10pt,y:9pt))[\n    #text(size: 7pt, fill: rgb(\"#94a3b8\"))[Nakit Ömrü]\n    #linebreak()\n    #text(size: 13pt, weight: \"bold\", fill: rgb(\"{risk_color}\"))[{runway}]\n    #linebreak()\n    #text(size: 7pt, fill: rgb(\"#64748b\"))[Risk: {risk_label}]\n  ],\n"
    );

    // Aylık tablo satırları
    let mut monthly_rows = String::with_capacity(monthly_data.len() * 200);
    for m in monthly_data {
        let ay = m["ay"].as_str().unwrap_or("-");
        let g  = m["gelir"].as_f64().unwrap_or(0.0);
        let e  = m["gider"].as_f64().unwrap_or(0.0);
        let n  = g - e;
        let (sign, nc) = if n >= 0.0 { ("+", "#16a34a") } else { ("", "#dc2626") };
        let status = if n >= 0.0 { "Pozitif" } else { "Negatif" };
        let sc = if n >= 0.0 { "#16a34a" } else { "#dc2626" };
        monthly_rows.push_str(&format!(
            "  [{ay}], [{g:.0}], [{e:.0}], [#text(fill: rgb(\"{nc}\"))[{sign}{n:.0}]], [#text(fill: rgb(\"{sc}\"), weight: \"bold\")[{status}]],\n"
        ));
    }

    // Yorum/tavsiye paragrafı
    let mut tavsiye = String::with_capacity(512);
    if net >= 0.0 {
        tavsiye.push_str("İşletme analiz döneminde pozitif net nakit akışı üretmiştir. Mevcut büyüme hızını korumak ve gider kontrolünü sürdürmek öncelikli hedef olmalıdır. ");
    } else {
        tavsiye.push_str("İşletme analiz döneminde negatif nakit akışı kaydetmiştir. Nakit akışını iyileştirmek için gelir kaynaklarının çeşitlendirilmesi ve gider kalemlerinin optimize edilmesi önerilmektedir. ");
    }
    tavsiye.push_str(&format!(
        "Mevcut Finansal Sağlık Skoru ({skor}/100) ve risk profili ({risk_label}) dikkate alındığında KOSGEB ve TÜBİTAK destek programlarından yararlanılması tavsiye edilmektedir."
    ));

    let gg_ratio = if total_gider > 0.0 { total_gelir / total_gider } else { 0.0 };

    // Tek pass'te tüm placeholder'ları değiştir
    TYPST_TEMPLATE
        .replace("{{DATE_RANGE}}", date_range)
        .replace("{{NOW}}", now)
        .replace("{{TOTAL_GELIR_F2}}", &format!("{:.2}", total_gelir))
        .replace("{{TOTAL_GIDER_F2}}", &format!("{:.2}", total_gider))
        .replace("{{TOTAL_GELIR}}", &format!("{:.0}", total_gelir))
        .replace("{{TOTAL_GIDER}}", &format!("{:.0}", total_gider))
        .replace("{{NET_F2}}", &format!("{:.2}", net))
        .replace("{{NET}}", &format!("{:.0}", net))
        .replace("{{NET_SIGN}}", net_sign)
        .replace("{{NET_COLOR}}", net_color)
        .replace("{{MONTHLY_GELIR_F2}}", &format!("{:.2}", monthly_gelir))
        .replace("{{MONTHLY_GIDER_F2}}", &format!("{:.2}", monthly_gider))
        .replace("{{SKOR_COLOR}}", skor_color)
        .replace("{{SKOR}}", &skor.to_string())
        .replace("{{HARF}}", harf)
        .replace("{{RUNWAY}}", runway)
        .replace("{{RISK_LABEL}}", risk_label)
        .replace("{{RISK_COLOR}}", risk_color)
        .replace("{{TREND}}", trend)
        .replace("{{GG_RATIO}}", &format!("{:.2}", gg_ratio))
        .replace("{{KPI_CARDS}}", &kpi_cards)
        .replace("{{MONTHLY_ROWS}}", &monthly_rows)
        .replace("{{TAVSIYE}}", &tavsiye)
}

/// POST /api/upload/csv — raw CSV body
pub async fn upload_csv_handler(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: Bytes,
) -> Result<impl IntoResponse, AeraError> {
    let start = Instant::now();

    let session_id = normalize_session_id(params.get("session_id").cloned())?;

    let income  = params.get("income_column").cloned();
    let expense = params.get("expense_column").cloned();
    let date    = params.get("date_column").cloned();

    let csv_str = std::str::from_utf8(&body)
        .map_err(|_| AeraError::BadRequest("CSV geçerli UTF-8 değil".into()))?;

    tracing::info!(
        "📤 [{}] CSV yükleniyor: {} byte",
        short_id(&session_id), body.len()
    );

    let user_key = resolve_api_key(&headers, &state.gemini_api_key)?;
    let orchestrator_arc = state.get_or_create_session(&session_id)?;
    let mut orchestrator = orchestrator_arc.lock().await;

    if let Some(key) = user_key {
        orchestrator.update_api_key(key);
    }

    let (rows, cols, col_names) = orchestrator
        .load_data_from_string(csv_str, income, expense, date)?;

    let date_range = orchestrator.engine.date_range.clone();
    let monthly_data = orchestrator.engine.monthly_breakdown();

    let latency_ms = start.elapsed().as_millis();
    tracing::info!("✅ [{}] {}ms | {}×{} | {} ay", short_id(&session_id), latency_ms, rows, cols, monthly_data.len());

    Ok(Json(UploadResponse {
        success: true,
        message: format!(
            "{} satır, {} sütun başarıyla yüklendi. Artık finansal analize hazırsınız.",
            rows, cols
        ),
        rows,
        columns: cols,
        column_names: col_names,
        date_range,
        monthly_data,
    }))
}

/// POST /api/upload/xlsx — Excel dosyası yükle (calamine ile CSV'ye dönüştür)
pub async fn upload_xlsx_handler(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: Bytes,
) -> Result<impl IntoResponse, AeraError> {
    use calamine::{Reader, Xlsx, Data};
    use std::io::Cursor;

    let start = Instant::now();

    let session_id = normalize_session_id(params.get("session_id").cloned())?;

    tracing::info!(
        "📊 [{}] Excel yükleniyor: {} byte",
        short_id(&session_id), body.len()
    );

    // Excel → CSV dönüşümü
    let cursor = Cursor::new(body.as_ref());
    let mut workbook: Xlsx<_> = Xlsx::new(cursor)
        .map_err(|e| AeraError::BadRequest(format!("Excel dosyası okunamadı: {e}")))?;

    let sheet_name = workbook.sheet_names().first()
        .ok_or_else(|| AeraError::BadRequest("Excel dosyasında sayfa bulunamadı".into()))?
        .clone();

    let range = workbook.worksheet_range(&sheet_name)
        .map_err(|e| AeraError::BadRequest(format!("Sayfa okunamadı: {e}")))?;

    // Range → CSV string
    let mut csv_buf = String::with_capacity(range.height() * 100);
    for (i, row) in range.rows().enumerate() {
        if i > 0 { csv_buf.push('\n'); }
        for (j, cell) in row.iter().enumerate() {
            if j > 0 { csv_buf.push(','); }
            match cell {
                Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => {
                    // Virgül veya tırnak içeriyorsa quote et
                    if s.contains(',') || s.contains('"') {
                        csv_buf.push('"');
                        csv_buf.push_str(&s.replace('"', "\"\""));
                        csv_buf.push('"');
                    } else {
                        csv_buf.push_str(s);
                    }
                }
                Data::Float(f) => csv_buf.push_str(&format!("{f}")),
                Data::Int(n) => csv_buf.push_str(&format!("{n}")),
                Data::DateTime(dt) => csv_buf.push_str(&format!("{dt}")),
                Data::Bool(b) => csv_buf.push_str(if *b { "true" } else { "false" }),
                Data::Error(e) => csv_buf.push_str(&format!("{e:?}")),
                Data::Empty => {}
            }
        }
    }

    let user_key = resolve_api_key(&headers, &state.gemini_api_key)?;
    let orchestrator_arc = state.get_or_create_session(&session_id)?;
    let mut orchestrator = orchestrator_arc.lock().await;

    if let Some(key) = user_key {
        orchestrator.update_api_key(key);
    }

    let income  = params.get("income_column").cloned();
    let expense = params.get("expense_column").cloned();
    let date    = params.get("date_column").cloned();

    let (rows, cols, col_names) = orchestrator
        .load_data_from_string(&csv_buf, income, expense, date)?;

    let date_range = orchestrator.engine.date_range.clone();
    let monthly_data = orchestrator.engine.monthly_breakdown();

    let latency_ms = start.elapsed().as_millis();
    tracing::info!("✅ [{}] Excel {}ms | {}×{} | {} ay", short_id(&session_id), latency_ms, rows, cols, monthly_data.len());

    Ok(Json(UploadResponse {
        success: true,
        message: format!(
            "Excel dosyası başarıyla yüklendi: {} satır, {} sütun. Finansal analize hazırsınız.",
            rows, cols
        ),
        rows,
        columns: cols,
        column_names: col_names,
        date_range,
        monthly_data,
    }))
}
