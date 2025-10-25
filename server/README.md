````markdown
# CappyCoding Metrics Server

A Go service that provides GitHub metrics for tracking pull requests, commits, and workflow runs.

## Prerequisites
- Go 1.21+
- GitHub personal access token with `repo` scope (required for private repositories)

## Authentication

The server supports three methods for providing a GitHub token (checked in priority order):

### 1. Environment Variable (Recommended for servers/CI)
```bash
export GITHUB_TOKEN="ghp_your_token_here"
```

### 2. Tauri Config File (Desktop app integration)
The server automatically checks these locations:
- `$APPDATA/com.sluggish-solutions.capycoding/config.json` (Windows)
- `$XDG_CONFIG_HOME/com.sluggish-solutions.capycoding/config.json` (Linux)
- `$HOME/.config/com.sluggish-solutions.capycoding/config.json` (macOS/Linux)

Config file format:
```json
{
  "githubToken": "ghp_your_token_here"
}
```

### 3. Per-Request Headers (No default token needed)
Include one of these headers with each request:
- `Authorization: Bearer <token>`
- `Authorization: token <token>`
- `X-GitHub-Token: <token>`

**Note:** If no default token is configured, the server will start but all requests must include authentication headers.

## Running the Server

```bash
cd server
go build -o server .
./server
```

Or run directly:
```bash
go run .
```

The server listens on `:8080` by default. Override with the `PORT` environment variable:
```bash
export PORT=":3000"
./server
```

## API Endpoints

### 1. Pull Requests
**Endpoint:** `GET /metrics/prs`

**Query Parameters:**
- `user` (required) - GitHub username
- `state` (optional) - Filter by state: `open`, `closed`, or `merged`
- `per_page` (optional) - Number of results (default: 20, max: 100)

**Examples:**
```bash
# Get all PRs for a user
curl "http://localhost:8080/metrics/prs?user=akhil-datla"

# Get only merged PRs
curl "http://localhost:8080/metrics/prs?user=akhil-datla&state=merged"

# Get open PRs
curl "http://localhost:8080/metrics/prs?user=akhil-datla&state=open"

# Get closed PRs (includes both merged and unmerged)
curl "http://localhost:8080/metrics/prs?user=akhil-datla&state=closed"

# Limit results
curl "http://localhost:8080/metrics/prs?user=akhil-datla&per_page=10"
```

**Response:**
```json
[
  {
    "number": 5,
    "title": "Add new feature",
    "state": "closed",
    "url": "https://github.com/owner/repo/pull/5",
    "updatedAt": "2025-10-25T09:55:22Z",
    "author": "akhil-datla",
    "merged": true
  }
]
```

**Note:** The `merged` field only appears when filtering by `state=merged`.

### 2. Commits
**Endpoint:** `GET /metrics/commits`

**Query Parameters:**
- `user` (required) - GitHub username
- `since` (optional) - Start date in RFC3339 format (e.g., `2025-10-25T00:00:00Z`)
- `until` (optional) - End date in RFC3339 format (e.g., `2025-10-25T23:59:59Z`)

**Examples:**
```bash
# Get all-time commit count
curl "http://localhost:8080/metrics/commits?user=akhil-datla"

# Get commits for today
curl "http://localhost:8080/metrics/commits?user=akhil-datla&since=2025-10-25T00:00:00Z"

# Get commits for a specific date range
curl "http://localhost:8080/metrics/commits?user=akhil-datla&since=2025-10-01T00:00:00Z&until=2025-10-31T23:59:59Z"

# Get commits for this week
curl "http://localhost:8080/metrics/commits?user=akhil-datla&since=2025-10-20T00:00:00Z"
```

**Response:**
```json
{
  "total": 937,
  "byAuthor": {
    "akhil-datla": 937
  },
  "since": "2025-10-25T00:00:00Z"
}
```

### 3. Workflow Runs
**Endpoint:** `GET /metrics/workflows`

**Query Parameters:**
- `user` (required) - GitHub username
- `branch` (optional) - Filter by branch name
- `per_page` (optional) - Number of results (default: 20, max: 100)

**Examples:**
```bash
# Get all workflow runs for a user
curl "http://localhost:8080/metrics/workflows?user=akhil-datla"

# Filter by branch
curl "http://localhost:8080/metrics/workflows?user=akhil-datla&branch=main"

# Limit results
curl "http://localhost:8080/metrics/workflows?user=akhil-datla&per_page=10"
```

**Response:**
```json
[
  {
    "id": 18121983554,
    "name": "CI Build",
    "status": "completed",
    "conclusion": "success",
    "htmlUrl": "https://github.com/owner/repo/actions/runs/18121983554",
    "createdAt": "2025-09-30T07:18:00Z",
    "updatedAt": "2025-09-30T07:18:51Z"
  }
]
```

## Using with Authentication Headers

If running without a default token:

```bash
# Start server
./server
# Output: "requests must include Authorization or X-GitHub-Token headers"

# Make authenticated requests
curl -H "Authorization: token ghp_your_token" \
  "http://localhost:8080/metrics/prs?user=akhil-datla"

# Or use X-GitHub-Token header
curl -H "X-GitHub-Token: ghp_your_token" \
  "http://localhost:8080/metrics/commits?user=akhil-datla"
```

## Testing

Run the test suite:
```bash
go test ./...
```

Run with verbose output:
```bash
go test ./... -v
```

## Development

Build the server:
```bash
go build -o server .
```

Run with debug logging:
```bash
./server 2>&1 | tee server.log
```

## Notes

- **Date Range Filtering:** When both `since` and `until` are provided for commits, the GitHub Search API uses range format (`YYYY-MM-DD..YYYY-MM-DD`) for accurate filtering.
- **Merged PRs:** The `state=closed` filter returns all closed PRs (both merged and unmerged). Use `state=merged` to get only merged PRs.
- **Rate Limits:** GitHub API has rate limits (5000 requests/hour for authenticated requests, 60 for unauthenticated). The server uses your token's quota.
- **Private Repos:** Requires a token with `repo` scope to access private repository data.

````
