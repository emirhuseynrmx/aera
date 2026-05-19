# syntax=docker/dockerfile:1.6
# ─────────────────────────────────────────────────────────────────────────────
# AeraCFO Dockerfile — multi-stage, slim runtime, rustls (no OpenSSL needed).
# Build: docker build -t aeracfo .
# Run:   docker run --rm -p 3000:3000 -e GEMINI_API_KEY=xxx aeracfo
# ─────────────────────────────────────────────────────────────────────────────

############
# 1) BUILD #
############
# Rust 1.85+ gerekli: bağımlılık ağacında hashbrown 0.17 edition2024 istiyor (1.85'te stabil).
# Sabit minör versiyon → reproducible build.
FROM rust:1.90-slim AS builder
WORKDIR /app

# Sadece pkg-config — reqwest=rustls-tls olduğu için libssl gerekmiyor.
# ca-certs build sırasında crates.io fetch için, tar tooling tamam.
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# 1a) Bağımlılık katmanı: yalnız Cargo manifest'lerini kopyala, dummy main üret,
#     böylece kaynak değişince crate cache yeniden indirilmiyor.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src src/bin \
 && echo 'fn main(){}' > src/main.rs \
 && echo 'fn main(){}' > src/bin/regenerate_demos.rs \
 && echo 'fn main(){}' > src/bin/score_demos.rs \
 && cargo build --release --bin aera_cfo \
 && rm -rf src

# 1b) Gerçek kaynak. `touch` ile main.rs zaman damgasını ileri al → cargo rebuild.
COPY src ./src
COPY data ./data
COPY templates ./templates
RUN touch src/main.rs && cargo build --release --bin aera_cfo

##############
# 2) RUNTIME #
##############
FROM debian:bookworm-slim AS runtime

# tini → PID 1 zombie reaper (cargo run ile aynı semantik).
# typst → PDF export endpoint'i bunsuz 500 döner.
# curl  → HEALTHCHECK için.
# ca-certificates → Gemini TLS doğrulaması.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates tini curl xz-utils \
 && ARCH=$(dpkg --print-architecture) \
 && case "$ARCH" in \
      amd64) TYPST_ARCH="x86_64-unknown-linux-musl" ;; \
      arm64) TYPST_ARCH="aarch64-unknown-linux-musl" ;; \
      *) echo "Unsupported arch: $ARCH" && exit 1 ;; \
    esac \
 && curl -fsSL "https://github.com/typst/typst/releases/download/v0.12.0/typst-${TYPST_ARCH}.tar.xz" \
      | tar -xJ -C /tmp \
 && mv /tmp/typst-${TYPST_ARCH}/typst /usr/local/bin/typst \
 && rm -rf /tmp/typst-* \
 && apt-get purge -y --auto-remove xz-utils curl \
 && apt-get install -y --no-install-recommends curl \
 && rm -rf /var/lib/apt/lists/*

# Non-root kullanıcı — exec'ler /tmp'ye yazıyor, HOME=/home/app sorun çıkmasın.
RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin app
WORKDIR /app

# Binary + runtime'da okunan asset'ler.
# Statik demo CSV'leri ve typst template runtime'da fs::read_to_string ile okunuyor.
COPY --from=builder /app/target/release/aera_cfo /usr/local/bin/aera_cfo
COPY --chown=app:app frontend ./frontend
COPY --chown=app:app data ./data
COPY --chown=app:app templates ./templates

USER app
ENV RUST_LOG=info \
    SERVER_ADDR=0.0.0.0:3000 \
    AERA_FRONTEND_DIR=/app/frontend \
    TYPST_BIN=/usr/local/bin/typst \
    HOME=/home/app

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3000/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["aera_cfo"]
