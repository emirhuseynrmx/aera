use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use crate::agents::orchestrator::CfoOrchestrator;
use crate::core::error::AeraError;

// Rate limit (DDoS / maliyet koruması)
const RATE_LIMIT_PER_MIN: u32 = 300;

// Maksimum eş zamanlı oturum sınırı (OOM koruması)
const MAX_CONCURRENT_SESSIONS: usize = 10_000;

// Inaktif IP'lerin haritadan silinme süresi (Memory leak koruması)
const RATE_LIMIT_ENTRY_TTL_SECS: u64 = 300;

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<DashMap<String, Arc<Mutex<CfoOrchestrator>>>>,
    pub gemini_api_key: Arc<String>,
    pub last_active: Arc<DashMap<String, Instant>>,
    // (istek_sayısı, pencere_başlangıcı) — sliding window rate limiter
    request_counts: Arc<DashMap<String, (u32, Instant)>>,
}

impl AppState {
    pub fn new(api_key: String) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            gemini_api_key: Arc::new(api_key),
            last_active: Arc::new(DashMap::new()),
            request_counts: Arc::new(DashMap::new()),
        }
    }

    /// IP bazlı sliding window rate limiter
    pub fn check_rate_limit(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut entry = self.request_counts
            .entry(key.to_string())
            .or_insert((0, now));
        let (count, window_start) = entry.value_mut();
        if window_start.elapsed().as_secs() >= 60 {
            *count = 0;
            *window_start = now;
        }
        *count += 1;
        *count <= RATE_LIMIT_PER_MIN
    }

    /// Yeni session oluşturma / varolanı getirme
    pub fn get_or_create_session(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<CfoOrchestrator>>, AeraError> {
        self.last_active.insert(session_id.to_string(), Instant::now());

        // Varolanı dön
        if let Some(existing) = self.sessions.get(session_id) {
            return Ok(existing.clone());
        }

        // Kapasite kontrolü
        if self.sessions.len() >= MAX_CONCURRENT_SESSIONS {
            return Err(AeraError::TooManyRequests(format!(
                "Sistem kapasitesi doldu ({} eş zamanlı oturum). Lütfen daha sonra tekrar deneyin.",
                MAX_CONCURRENT_SESSIONS
            )));
        }

        let orchestrator = self
            .sessions
            .entry(session_id.to_string())
            .or_insert_with(|| {
                tracing::info!("🆕 Yeni session: {}", short_id(session_id));
                Arc::new(Mutex::new(CfoOrchestrator::new(
                    (*self.gemini_api_key).clone(),
                )))
            })
            .clone();
        Ok(orchestrator)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Süresi dolan session ve rate-limitleri temizle (GC)
    pub fn cleanup_expired_sessions(&self, ttl_secs: u64) {
        let now = Instant::now();

        // Session GC
        let expired: Vec<String> = self.last_active
            .iter()
            .filter(|r| now.duration_since(*r.value()).as_secs() > ttl_secs)
            .map(|r| r.key().clone())
            .collect();
        for key in expired {
            self.sessions.remove(&key);
            self.last_active.remove(&key);
            tracing::info!("🗑️ Session TTL doldu: {}", short_id(&key));
        }

        // Inaktif rate-limit IP'lerini sil
        let stale: Vec<String> = self.request_counts
            .iter()
            .filter(|r| r.value().1.elapsed().as_secs() > RATE_LIMIT_ENTRY_TTL_SECS)
            .map(|r| r.key().clone())
            .collect();
        let stale_len = stale.len();
        for k in stale { self.request_counts.remove(&k); }
        if stale_len > 0 {
            tracing::debug!("🧹 Rate-limit GC: {} eski kayıt silindi", stale_len);
        }
    }
}

/// UTF-8 güvenli session ID kısaltıcı
#[inline]
pub fn short_id(s: &str) -> String {
    s.chars().take(8).collect()
}
