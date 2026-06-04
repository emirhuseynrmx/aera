# AeraCFO

Autonomous AI CFO platform for SMEs, built with Rust, Axum, Polars, and Gemini.

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/API-Axum-blue)](https://github.com/tokio-rs/axum)
[![Polars](https://img.shields.io/badge/Analytics-Polars-teal)](https://www.pola.rs/)
[![Gemini](https://img.shields.io/badge/AI-Gemini%20Function%20Calling-purple)](https://ai.google.dev/)

![AeraCFO interface](docs/download.gif)

AeraCFO turns SME financial data into CFO-style decisions. Upload a CSV or XLSX file, ask a business question, and the system routes the request through an agent pipeline that can analyze cash flow, score financial health, simulate what-if scenarios, search incentives, and generate an executive PDF report.

> A larger AeraCFO update is planned soon, focused on production-grade reporting, stronger agent reliability, improved data ingestion, and a more polished decision workflow.

## Why It Exists

Most small businesses do not need another static dashboard. They need fast answers to questions like:

- How risky is my current cash position?
- What happens if I hire two more people?
- Which month creates the cash crunch?
- Which incentive programs might fit this company?
- Can I export a clear report for a stakeholder?

AeraCFO is designed around those decision moments instead of raw chart collection.

## Core Capabilities

| Area | What AeraCFO does |
| --- | --- |
| Agent workflow | Planner, tool executor, coordinator, and critic-style review around CFO questions. |
| Financial analytics | Cash flow, burn rate, runway, health score, category analysis, anomaly detection, and projections. |
| Scenario modeling | What-if simulation for extra income, extra expense, and short-horizon planning. |
| Data ingestion | CSV and XLSX upload, demo datasets, and live synthetic demo generation by sector/pattern. |
| Reporting | Typst-backed PDF export with KPI cards, monthly breakdowns, and risk summaries. |
| Frontend | Static browser UI served directly by the Rust backend. |

## Architecture

```text
User question / uploaded data
        |
        v
Axum API + session state
        |
        v
Agent coordinator
        |
        +-- Planner
        +-- CFO tools
        +-- Gemini function calling
        +-- Critic / response shaping
        |
        v
Polars analytics engine
        |
        +-- Cash-flow metrics
        +-- Health scoring
        +-- Forecasts
        +-- Scenario simulation
        +-- Incentive search
        |
        v
Frontend response / PDF report
```

## Project Structure

```text
.
|-- src/
|   |-- api/              # Axum routes, schemas, server state
|   |-- agents/           # Planner, coordinator, orchestrator, tool declarations
|   |-- core/             # Config and error types
|   |-- infrastructure/   # Polars engine, Gemini client, incentives DB, demo generator
|   `-- bin/              # Utility binaries for demo data
|-- frontend/             # Static browser UI served by the backend
|-- data/                 # Demo CSVs and incentive reference data
|-- docs/                 # Screenshots, GIFs, and visual documentation assets
|-- templates/            # Typst report template
|-- Dockerfile
|-- docker-compose.yml
`-- README.tr.md          # Turkish README archive
```

## API Surface

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Runtime health check and active session count. |
| `POST` | `/api/chat` | Main conversational CFO endpoint. |
| `POST` | `/api/upload/csv` | Upload raw CSV data into a session. |
| `POST` | `/api/upload/xlsx` | Upload Excel data into a session. |
| `GET` | `/api/demo` | Load a static or generated demo company dataset. |
| `GET` | `/api/export/pdf` | Export a Typst-generated PDF report for a session. |

## CFO Tools

The Gemini tool layer currently exposes:

- `analyze_cash_flow`
- `get_data_summary`
- `search_incentives`
- `predict_cashflow`
- `get_health_score`
- `compare_sector_benchmark`
- `analyze_expense_categories`
- `detect_anomalies`
- `simulate_scenario`
- `detect_cash_crunch`

## Quick Start

### 1. Configure environment

```bash
cp .env.example .env
```

Set your Gemini key:

```bash
GEMINI_API_KEY=your_gemini_api_key_here
SERVER_ADDR=0.0.0.0:3000
```

### 2. Run locally

```bash
cargo run
```

Open:

```text
http://localhost:3000
```

### 3. Run with Docker

```bash
docker compose up --build
```

## Useful Commands

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo run --bin regenerate_demos
cargo run --bin score_demos
```

## Environment Variables

| Variable | Required | Description |
| --- | --- | --- |
| `GEMINI_API_KEY` | Yes | Gemini API key used by the agent layer. |
| `SERVER_ADDR` | No | Bind address. Defaults to `0.0.0.0:3000`. |
| `GEMINI_MODEL` | No | Optional Gemini model override. |
| `ALLOWED_ORIGINS` | No | Comma-separated CORS allowlist. |
| `TRUST_PROXY_HEADERS` | No | Enables proxy IP headers when set to `1` or `true`. |
| `AERA_FRONTEND_DIR` | No | Static frontend directory. Defaults to `frontend`. |
| `TYPST_BIN` | No | Typst binary path for PDF export. Defaults to `typst`. |

## Upcoming Update

AeraCFO is being prepared for a larger update. The next version is expected to improve:

- Report quality and stakeholder-ready PDF output.
- Agent evaluation, fallback behavior, and answer consistency.
- Data ingestion for more realistic SME finance files.
- UI polish around what-if simulations, dashboards, and export flows.
- Production packaging and deployment ergonomics.

## License

This project is licensed under the [MIT License](LICENSE).

For the archived Turkish README, see [README.tr.md](README.tr.md).
