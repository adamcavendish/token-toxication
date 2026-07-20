# Token Toxication

[![CI](https://github.com/token-toxication/token-toxication/actions/workflows/ci.yml/badge.svg)](https://github.com/token-toxication/token-toxication/actions/workflows/ci.yml)

**A self-hosted control plane and relay for Anthropic, OpenAI, and Gemini-compatible AI providers.**

Token Toxication gives clients one stable API surface while operators manage provider credentials, exact model mappings, primary and backup routes, health state, request logs, and token usage from a web console.

The application ships as a Rust server with an embedded React admin interface and SQLite persistence.

## Features

| Capability | Description |
| --- | --- |
| Multi-protocol relay | Anthropic Messages, OpenAI Chat Completions, OpenAI Responses, and Gemini GenerateContent endpoints |
| Model routing | Exact public-to-upstream model mappings with primary and backup routes |
| Provider management | Per-account credentials, base URLs, wire protocols, priority, and health state |
| Reliability controls | Account blocking, route cooldowns, and automatic selection of the next eligible route |
| Usage analytics | Provider-reported input, cached-input, and output token accounting |
| Privacy-conscious logs | Operational metadata without prompts, messages, credentials, or raw upstream bodies |
| Self-hosted administration | API-key management, provider setup, route configuration, quotas, and request logs |
| Client setup | Generated configurations for Codex, Claude Code, opencode, and Pi |
| Production TLS | Plain HTTP, certificate files, or managed ACME HTTP-01 certificates |

## Contents

- [Quick start](#quick-start)
- [Relay API](#relay-api)
- [Routing model](#routing-model)
- [Provider setup](#provider-setup)
- [Reliability](#reliability)
- [Usage analytics and privacy](#usage-analytics-and-privacy)
- [Configuration and TLS](#configuration-and-tls)
- [Development](#development)

## Quick start

### Prerequisites

- Rust 1.88 or newer
- [just](https://github.com/casey/just)
- Vite+ with the `vp` command available
- `openapi-nexus` for SDK generation

### Run locally

```bash
just ui-install
just sdk-generate
just ui-build

TT_BIND_ADDR=127.0.0.1:3000 \
TT_ADMIN_PASSWORD='choose-a-strong-password' \
just dev-server
```

Open [http://localhost:3000](http://localhost:3000) and sign in as `admin` with the password supplied through `TT_ADMIN_PASSWORD`.

> [!IMPORTANT]
> Set a strong admin password before exposing the service. Provider credentials are stored in the local SQLite database, so protect `data/` and its backups.

### Configure the first model

Use the admin interface to create these records in order:

1. A **provider account** containing the upstream URL, protocol, and credential.
2. A **catalog model** containing the public model ID clients will request.
3. A **provider model route** mapping that public ID to the upstream model ID.
4. A **relay API key** for client authentication.

The model becomes discoverable after the catalog entry, route, and provider account are enabled and eligible.

### Connect a client

The **Client Setup** page generates copy-ready configuration for Codex, Claude Code, opencode, and Pi from eligible routes.

Codex and Pi use Responses routes, Claude Code uses Anthropic Messages routes, and opencode selects the AI SDK that matches each configured OpenAI route. Pi setup writes a complete `~/.pi/agent/models.json` file, so back up any existing Pi configuration first. It references the relay key through `TOKEN_TOXICATION_API_KEY`; do not commit a literal key in that file.

## Relay API

### Endpoints

| Protocol | Method | Endpoint | Recommended client base URL |
| --- | --- | --- | --- |
| Anthropic Messages | `POST` | `/anthropic/v1/messages` | `http://localhost:3000/anthropic` |
| OpenAI Chat | `POST` | `/openai/v1/chat/completions` | `http://localhost:3000/openai/v1` |
| OpenAI Responses | `POST` | `/openai/v1/responses` | `http://localhost:3000/openai/v1` |
| Gemini GenerateContent | `POST` | `/gemini/v1beta/models/{model}:generateContent` | `http://localhost:3000/gemini` |
| Gemini streaming | `POST` | `/gemini/v1beta/models/{model}:streamGenerateContent` | `http://localhost:3000/gemini` |

Model discovery is available at `/anthropic/v1/models`, `/openai/v1/models`, and `/gemini/v1beta/models`. Individual model lookups use the corresponding `/models/{model}` path.

### Service endpoints

| Endpoint | Description |
| --- | --- |
| `/health` | Service status, version, uptime, and timestamp |
| `/metrics` | JSON totals for API keys, provider health, and usage |
| `/openapi.json` | OpenAPI document for the admin and relay APIs |

### Authentication

Relay API keys use the configured prefix, `tokentoxication-` by default. Send a key through any supported client convention:

- `Authorization: Bearer <key>`
- `x-api-key: <key>`
- `x-goog-api-key: <key>`
- `api-key: <key>`
- Gemini's `?key=<key>` query parameter

### Anthropic Messages

```bash
curl http://localhost:3000/anthropic/v1/messages \
  -H 'x-api-key: tokentoxication-...' \
  -H 'content-type: application/json' \
  -d '{
    "model": "claude-sonnet-4-5",
    "max_tokens": 64,
    "messages": [{"role": "user", "content": "hello"}]
  }'
```

### OpenAI Chat

```bash
curl http://localhost:3000/openai/v1/chat/completions \
  -H 'Authorization: Bearer tokentoxication-...' \
  -H 'content-type: application/json' \
  -d '{
    "model": "deepseek-v4-pro",
    "messages": [{"role": "user", "content": "hello"}]
  }'
```

### OpenAI Responses

```bash
curl http://localhost:3000/openai/v1/responses \
  -H 'Authorization: Bearer tokentoxication-...' \
  -H 'content-type: application/json' \
  -d '{"model": "gpt-5", "input": "hello"}'
```

### Gemini GenerateContent

```bash
curl http://localhost:3000/gemini/v1beta/models/gemini-3.1-pro-high:generateContent \
  -H 'x-goog-api-key: tokentoxication-...' \
  -H 'content-type: application/json' \
  -d '{"contents": [{"parts": [{"text": "hello"}]}]}'
```

## Routing model

Every relay request resolves through three records:

| Record | Responsibility |
| --- | --- |
| Provider account | Owns the upstream base URL, credential, authentication mode, wire protocol, priority, and account health |
| Catalog model | Defines the exact, case-preserving model ID exposed to clients |
| Provider model route | Maps a public model to one account and upstream model ID, with a primary or backup role |

There is no global upstream URL. Each account owns its connection settings, and each route can remove configured top-level request fields before forwarding.

Routing policy uses the top-level model ID and configured `stripParams`. It does not inspect nested prompts, messages, or input content. Provider adapters handle protocol-specific transformations separately.

Eligible primary routes are selected before backup routes. Within the same role, higher-priority accounts are preferred and least-recently-used routes rotate first.

Each request receives one upstream attempt. A failure does not trigger another provider in the same request; later requests skip accounts or routes that are blocked or cooling down.

### Wire protocols

| Wire API | Upstream request |
| --- | --- |
| `anthropic-messages` | `{base_url}/v1/messages` |
| `openai-chat` | `{base_url}/chat/completions` |
| `openai-responses` | `{base_url}/v1/responses` |
| `gemini-generate-content` | Gemini Code Assist GenerateContent or streaming endpoint |

If a base URL already ends with the required API version, the relay avoids duplicating that version segment.

## Provider setup

### Built-in presets

The admin interface loads provider presets from the backend. Presets supply the expected base URL, authentication mode, wire protocol, credential guidance, and provider aliases.

| Wire protocol | Included presets |
| --- | --- |
| Anthropic Messages | Anthropic, MiniMax, MiniMax Token Plan, and China-region MiniMax variants |
| OpenAI Responses | OpenAI API key and Codex subscription |
| OpenAI Chat | DeepSeek v4, Qwen, Kimi, Moonshot AI, Z.AI, Zhipu AI, coding-plan variants, and China-region variants |
| Gemini GenerateContent | Antigravity OAuth |

### OpenAI API keys

Choose **OpenAI API key**, keep `https://api.openai.com` as the base URL, and use the `openai-responses` wire protocol. Add catalog models and routes for the exact upstream model IDs you intend to expose.

### Codex subscriptions

Create one **Codex subscription** account per ChatGPT Plus or Pro subscription. Use `https://chatgpt.com/backend-api`, `codex-oauth`, and the `openai-responses` wire protocol.

Paste only the raw refresh token. Codex CLI stores it in `~/.codex/auth.json` at `tokens.refresh_token`; opencode stores it in `~/.local/share/opencode/auth.json` at `openai.refresh`.

Subscription requests are sent to `{base_url}/codex/responses`, while quota data comes from `{base_url}/wham/usage`. Legacy base URLs ending in `/codex` or `/codex/responses` are normalized automatically.

Codex subscription routes remove `max_output_tokens` to match the Codex CLI request shape. OpenAI API-key routes continue forwarding that field.

Configure Codex clients with base URL `http://localhost:3000/openai/v1`, the Responses API, and a generated relay key.

### OpenAI-compatible providers

Use `openai-chat` for providers such as DeepSeek, Qwen, Kimi, Moonshot AI, Z.AI, and Zhipu AI. Create exact catalog IDs and map each route to the model ID expected by its account.

OpenAI-compatible clients use base URL `http://localhost:3000/openai/v1` and change only the requested model name.

### Anthropic-compatible providers

Use `anthropic-messages` for Anthropic, MiniMax, and MiniMax token-plan accounts. The built-in presets include global and China-region MiniMax base URLs.

Anthropic-compatible clients use base URL `http://localhost:3000/anthropic`.

### Gemini with Antigravity OAuth

Set `TT_ANTIGRAVITY_OAUTH_CLIENT_SECRET`, choose **Antigravity OAuth**, and select **Sign in with Antigravity** in the admin interface.

The backend runs the PKCE OAuth flow, receives the loopback callback at `/oauth-callback`, and stores the resulting tokens without returning them to the browser.

A public OAuth client ID is built in. Set `TT_ANTIGRAVITY_OAUTH_CLIENT_ID` only when using a different registered client.

After sign-in, inspect the account's available models and quota data. Create routes with the exact upstream IDs returned for that account rather than relying on a fixed model alias.

Gemini clients may send native `contents`, `generationConfig`, `safetySettings`, `tools`, and `toolConfig` fields. Client authentication is consumed by the relay and is not forwarded to Google.

## Reliability

Provider account health and route health are tracked separately.

| Upstream outcome | Recorded behavior |
| --- | --- |
| HTTP 401 or 403 | Block the provider account |
| HTTP 429 | Cool the selected route for 60 seconds |
| MiniMax model quota reaches zero | Cool the selected route for one hour |
| HTTP 5xx or transport failure | Cool the selected route for 30 seconds |
| Responses `server_error` event | Record 502 and cool the route for 30 seconds |
| Responses `rate_limit_exceeded` event | Record 429 and cool the route for 60 seconds |
| Unknown Responses failure code | Record 502 without changing route health |

OpenAI Responses streams can fail after the upstream has already returned HTTP 200. The relay recognizes terminal `error` and `response.failed` events and records the attempt as failed.

The original response stream is forwarded unchanged. Failure classification affects route health and request logs, not the bytes returned to the client.

## Usage analytics and privacy

### Request logs

Request logs store the operational metadata required to diagnose routing and provider behavior:

- Upstream origin and path without query parameters
- Top-level request keys and body size
- Streaming mode
- Public and upstream model IDs
- Stripped parameter names
- Status, latency, error category, and provider account

Request logs do not store:

- Prompt, message, or input content
- Authorization headers or API keys
- Query parameters
- Raw request or upstream response bodies
- Provider-controlled failure messages

### Token and cache accounting

When a provider reports usage, the relay stores input, cached-input, and output token counts. It does not estimate or locally tokenize requests.

Streaming logs are finalized after the upstream stream ends so terminal usage events are captured.

```text
cache hit rate = cached input tokens / input tokens
```

The dashboard shows today's aggregate cache hit rate and the rate for each request. Older records and providers without cached-token reporting display `0%`.

Failed streams retain provider-reported usage when present. If the provider sends no usage, the token fields remain zero and the request status and error identify the failed attempt.

## Configuration and TLS

Every server option is available as a command-line flag and, where listed, an environment variable. Run the server with `--help` for the complete mapping.

### Core settings

| Environment variable | Default | Purpose |
| --- | --- | --- |
| `TT_BIND_ADDR` | `0.0.0.0:3000` | Main HTTP or HTTPS listener |
| `TT_DATABASE_PATH` | `data/token-toxication.sqlite3` | SQLite database path |
| `TT_STATIC_DIR` | `apps/admin/dist` | Optional external admin build; embedded assets are the fallback |
| `TT_ADMIN_USERNAME` | `admin` | Admin login username |
| `TT_ADMIN_PASSWORD` | `change-this-password` | Admin login password; change before deployment |
| `TT_API_KEY_PREFIX` | `tokentoxication-` | Prefix for generated relay keys |
| `RUST_LOG` | Server and HTTP info logs | Tracing filter |

### TLS modes

| Mode | Behavior |
| --- | --- |
| `off` | Serve plain HTTP on `TT_BIND_ADDR` |
| `cert-files` | Serve HTTPS from an existing certificate and private key |
| `acme-http-01` | Obtain and renew certificates through ACME HTTP-01 |

Use existing certificate files:

```bash
token-toxication-server \
  --bind-addr 0.0.0.0:443 \
  --https-mode cert-files \
  --tls-cert-path /etc/token-toxication/fullchain.pem \
  --tls-key-path /etc/token-toxication/privkey.pem
```

Manage a certificate through ACME:

```bash
token-toxication-server \
  --bind-addr 0.0.0.0:443 \
  --https-mode acme-http-01 \
  --acme-identifier relay.example.com \
  --acme-email ops@example.com \
  --acme-http-bind-addr 0.0.0.0:80 \
  --acme-cert-dir /var/lib/token-toxication/acme
```

HTTP-01 validation requires the challenge listener to be reachable on public port 80. Use `--acme-allow-nonstandard-http-port` only when an explicit forwarding layer maps public port 80 to another local port.

ACME accepts a domain or public IP address. IP certificates use the short-lived profile and renew aggressively. Account data, certificates, keys, and metadata remain under the configured certificate directory.

When systemd runs the service as a non-root user on ports 80 or 443, grant only the low-port bind capability:

```ini
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
```

## Development

### Repository layout

| Path | Purpose |
| --- | --- |
| `crates/token-toxication-server` | Axum server, routing policy, SQLite persistence, OpenAPI, and embedded admin assets |
| `apps/admin` | React, TypeScript, Vite+, and shadcn admin interface |
| `Justfile` | Local development, generation, and CI commands |
| `.github/workflows/ci.yml` | Rust and admin CI jobs |

### Common commands

| Command | Purpose |
| --- | --- |
| `just dev-server` | Run the backend and serve the built admin interface |
| `just ui-dev` | Run the admin development server |
| `just fmt` | Format the Rust workspace |
| `just clippy` | Run Clippy with warnings denied |
| `just test` | Run the Rust test suite |
| `just openapi-generate` | Generate the OpenAPI document |
| `just sdk-generate` | Generate and format the TypeScript admin SDK |
| `just ui-check` | Run frontend formatting, lint, and type checks |
| `just ui-build` | Build the admin interface |
| `just ci` | Run the full local CI pipeline |

The generated OpenAPI document is written to `openapi/token-toxication.openapi.json`. The generated TypeScript SDK is written to `apps/admin/src/generated/token-toxication`.

Both generated paths are ignored by Git and can be recreated with `just sdk-generate`.
