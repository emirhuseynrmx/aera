use aera_cfo::core::config::AppConfig;
use aera_cfo::api::server;

#[tokio::main]
async fn main() {
    // Kurumsal loglama altyapısını başlat
    tracing_subscriber::fmt()
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .init();

    tracing::info!("🚀 AeraCFO Finansal Ajan Motoru başlatılıyor...");
    tracing::info!("   Versiyon: {}", env!("CARGO_PKG_VERSION"));
    tracing::info!("   Motor: Rust/Axum + Polars + Gemini 2.0 Flash");

    // .env'den yapılandırmayı yükle
    let config = match AppConfig::load() {
        Ok(config) => {
            tracing::info!("✅ Yapılandırma yüklendi");
            config
        }
        Err(e) => {
            tracing::error!("❌ Yapılandırma hatası: {}", e);
            tracing::error!("   .env dosyanızda GEMINI_API_KEY tanımlı olduğundan emin olun.");
            std::process::exit(1);
        }
    };

    // Axum sunucusunu başlat
    if let Err(e) = server::start_server(&config).await {
        tracing::error!("❌ Sunucu hatası: {}", e);
        std::process::exit(1);
    }
}