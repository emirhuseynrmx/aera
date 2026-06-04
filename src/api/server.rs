use crate::api::{routes, state::AppState};
use crate::core::config::AppConfig;
use crate::core::error::AeraResult;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

pub async fn start_server(config: &AppConfig) -> AeraResult<()> {
    let state = AppState::new(config.gemini_api_key.clone());

    // Session temizliği (memory leak koruması)
    let state_ttl = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            state_ttl.cleanup_expired_sessions(30 * 60);
        }
    });

    // CORS ayarları (ENV veya localhost)
    let cors_origins = std::env::var("ALLOWED_ORIGINS").ok();
    let origin_list: Vec<String> = match &cors_origins {
        Some(s) if !s.trim().is_empty() => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        _ => vec![
            "http://localhost:3000".into(),
            "http://127.0.0.1:3000".into(),
            "http://0.0.0.0:3000".into(),
            "http://localhost:8080".into(),
        ],
    };
    let allowed_origins: Vec<HeaderValue> =
        origin_list.iter().filter_map(|o| o.parse().ok()).collect();
    tracing::info!("🛡️  CORS izinli origin'ler: {:?}", origin_list);

    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-api-key"),
        ]);

    // Frontend dizini
    let frontend_dir =
        std::env::var("AERA_FRONTEND_DIR").unwrap_or_else(|_| "frontend".to_string());
    tracing::info!("📁 Frontend dizini: {}", frontend_dir);

    // Static file fallback
    let app = Router::new()
        .route("/health", get(routes::health_check))
        .route("/api/chat", post(routes::chat_handler))
        .route("/api/upload/csv", post(routes::upload_csv_handler))
        .route("/api/upload/xlsx", post(routes::upload_xlsx_handler))
        .route("/api/demo", get(routes::demo_handler))
        .route("/api/export/pdf", get(routes::export_pdf_handler))
        .with_state(state)
        .layer(cors)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .fallback_service(ServeDir::new(frontend_dir));

    let listener = tokio::net::TcpListener::bind(&config.server_addr)
        .await
        .map_err(|e| {
            crate::core::error::AeraError::AgentExecutionError(format!(
                "Port bağlanamadı {}: {}",
                config.server_addr, e
            ))
        })?;

    tracing::info!("🌐 AeraCFO → http://{}", config.server_addr);
    tracing::info!("   GET  /health");
    tracing::info!("   POST /api/chat");
    tracing::info!("   POST /api/upload/csv");

    // Client IP takibi için enjeksiyon
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| {
        crate::core::error::AeraError::AgentExecutionError(format!("Sunucu çöktü: {}", e))
    })?;

    Ok(())
}
