# Deployment

Deployment-related files live here so the repository root stays focused on the Rust crate and product entry points.

## Docker

Run from the repository root:

```bash
cp config/.env.example .env
docker compose -f deploy/docker/docker-compose.yml up --build
```

The compose file keeps the build context at the repository root, so the Rust source, frontend, data fixtures, and Typst templates are copied exactly as before.
