# Tauri + SvelteKit + TypeScript

This template should help get you started developing with Tauri, SvelteKit and TypeScript in Vite.

## Metrics server

A lightweight Go service located in `../server` exposes GitHub metrics that the
CapyCoding UI can consume.

### Prerequisites

Provide a GitHub personal access token either by exporting it as
`GITHUB_TOKEN` or by adding a JSON file at one of the following locations
(`githubToken` or `github.token` field):

* `%APPDATA%/com.sluggish-solutions.capycoding/config.json`
* `$XDG_CONFIG_HOME/com.sluggish-solutions.capycoding/config.json`
* `$HOME/.config/com.sluggish-solutions.capycoding/config.json`

### Running the server

```bash
cd ../server
go run ./...
```

By default the server listens on `:8080`. Override the bind address by setting
`CAPYCODING_SERVER_ADDR`.

Available HTTP endpoints:

* `GET /metrics/prs?owner=<org>&repo=<repo>&state=<state>` – recent pull
  requests.
* `GET /metrics/workflows?owner=<org>&repo=<repo>&branch=<branch>` – recent
  workflow runs for the repository and optional branch.
* `GET /metrics/commits?owner=<org>&repo=<repo>&since=<RFC3339>&until=<RFC3339>`
  – commit counts grouped by author.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
