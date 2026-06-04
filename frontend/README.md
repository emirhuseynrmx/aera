# Frontend

This folder contains the static browser UI served by the Rust backend.

The server exposes it through Axum's static fallback service. By default, the backend serves this directory:

```bash
AERA_FRONTEND_DIR=frontend
```

During local development, start the backend and open:

```text
http://localhost:3000
```

The current frontend is intentionally colocated with the backend so the product can run as a single deployable service.
