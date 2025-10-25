# CappyCoding Metrics Server

This Go service exposes GitHub metrics for the CappyCoding mascot.

## Prerequisites
- Go 1.21+
- A GitHub personal access token with `repo` and `workflow` scopes, provided via the `GITHUB_TOKEN` environment variable or the Tauri config file (`com.sluggish-solutions.capycoding/config.json`).

## Running the server
```bash
cd server
GITHUB_TOKEN=your_token go run ./...
```

By default the API listens on `:8080`. Override with the `CAPYCODING_SERVER_ADDR` environment variable.

## Available endpoints
- `GET /metrics/prs` – Latest pull requests.
- `GET /metrics/workflows` – Recent workflow runs.
- `GET /metrics/commits` – Commit activity summary.

All endpoints accept `owner` and `repo` query parameters. Optional filters:
- `/metrics/prs`: `state`, `per_page`
- `/metrics/workflows`: `branch`, `per_page`
- `/metrics/commits`: `since`, `until` (RFC 3339 timestamps)
