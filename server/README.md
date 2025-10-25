# CappyCoding Metrics Server

This Go service exposes GitHub metrics for the CappyCoding mascot.

## Prerequisites
- Go 1.21+
- (Optional) A GitHub personal access token with `repo` and `workflow` scopes. The backend discovers it from the
  `GITHUB_TOKEN` environment variable, the Tauri config file (`com.sluggish-solutions.capycoding/config.json`), or an
  override supplied by the frontend per request (see [Authenticating requests](#authenticating-requests)).

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

## Authenticating requests

When the Tauri frontend stores a GitHub token, it can forward it to the Go service without persisting it on the server. Each
request may include either of the following headers:

- `Authorization: Bearer <token>` (also accepts the legacy `token <token>` scheme).
- `X-GitHub-Token: <token>`

If no override header is present, the server falls back to the token discovered from the environment or Tauri config file.
When neither source provides credentials the server still starts, but callers must always supply a token via the headers
above.
