# Token Toxication

Token Toxication is a Rust + React relay service. It provides self-hosted
Anthropic Messages, OpenAI Responses, OpenAI Chat, and Gemini GenerateContent
relay endpoints, admin-managed API keys, provider account rotation, request
logging, provider-reported token and cache analytics, and a Vite+ shadcn
dashboard.

## Layout

- `crates/token-toxication-server` - Axum backend and SQLite persistence.
- `apps/admin` - React + Vite+ + shadcn admin UI.
- `.github/workflows/ci.yml` - Rust and Vite+ CI.

## Quick Start

```bash
cd apps/admin && vp install
cd ../..
just sdk-generate
just ui-build
cargo run -p token-toxication-server --bin token-toxication-server
```

Open `http://localhost:3000` and sign in with the configured admin username and
password. Add a provider account, catalog model, and provider model route before
using the relay.

Anthropic Messages relay:

```bash
curl http://localhost:3000/anthropic/v1/messages \
  -H "x-api-key: tokentoxication-..." \
  -H "content-type: application/json" \
  -d '{"model":"claude-sonnet-4-5","max_tokens":64,"messages":[{"role":"user","content":"hello"}]}'
```

OpenAI Chat relay, suitable for DeepSeek v4 accounts:

```bash
curl http://localhost:3000/openai/v1/chat/completions \
  -H "Authorization: Bearer tokentoxication-..." \
  -H "content-type: application/json" \
  -d '{"model":"deepseek-v4-pro","messages":[{"role":"user","content":"hello"}]}'
```

OpenAI Responses relay, suitable for Codex:

```bash
curl http://localhost:3000/openai/v1/responses \
  -H "Authorization: Bearer tokentoxication-..." \
  -H "content-type: application/json" \
  -d '{"model":"gpt-5","input":"hello"}'
```

Gemini native GenerateContent relay through a Gemini account:

```bash
curl http://localhost:3000/gemini/v1beta/models/gemini-3.1-pro-high:generateContent \
  -H "Authorization: Bearer tokentoxication-..." \
  -H "content-type: application/json" \
  -d '{"contents":[{"parts":[{"text":"hello"}]}]}'
```

Model discovery uses the same relay API key authentication. Concrete model names
come from enabled catalog entries with at least one active provider model route:

```bash
curl http://localhost:3000/openai/v1/models \
  -H "Authorization: Bearer tokentoxication-..."
```

Gemini native model discovery is also available:

```bash
curl http://localhost:3000/gemini/v1beta/models \
  -H "Authorization: Bearer tokentoxication-..."
```

## Provider Accounts and Model Routes

Each provider account has a `wire_api` protocol:

- `anthropic-messages` forwards to `{base_url}/v1/messages`.
- `openai-chat` forwards to `{base_url}/chat/completions`.
- `openai-responses` forwards to `{base_url}/v1/responses`.
- `gemini-generate-content` forwards to
  `{base_url}/v1internal:generateContent` and
  `{base_url}/v1internal:streamGenerateContent?alt=sse` using a refreshed
  Gemini account OAuth bearer token. The upstream model ID is sent in the Code
  Assist request body. Antigravity OAuth accounts use the native agent request
  envelope and Antigravity user agent expected by this backend.

For Anthropic-compatible clients, use base URL `http://localhost:3000/anthropic`.

There is no global upstream base URL. Each provider account owns its base URL,
auth mode, wire protocol, priority, and health state. Model names live in the
model catalog, and provider model routes bind each public model name to a
provider account plus exact upstream model name.

Provider presets are served by the backend at `/admin/api/provider-presets` and
are loaded by the admin UI. The same backend catalog also normalizes provider
aliases such as Kimi, Moonshot, Z.AI, Zhipu, MiniMax token-plan, DeepSeek v4,
Qwen/DashScope, and Gemini/Google AI names.

For OpenAI-compatible chat providers such as DeepSeek, Qwen, Kimi, Moonshot AI,
Z.AI, and Zhipu AI, create provider accounts with `openai-chat`, add catalog
models such as `deepseek-v4-pro`, `glm-5.2`, or `k2p6`, then create primary and
backup routes for those models. Clients can keep using
`/openai/v1/chat/completions` and switch only the model name.

MiniMax and MiniMax token-plan accounts use Anthropic-compatible endpoints in
opencode. Add them with `anthropic-messages` and base URLs such as
`https://api.minimax.io/anthropic/v1` or
`https://api.minimaxi.com/anthropic/v1`, then add exact catalog models such as
`MiniMax-M3` or `MiniMax-M2.7`.

For Gemini native GenerateContent, set
`TT_ANTIGRAVITY_OAUTH_CLIENT_SECRET` on the backend, then choose the
`Antigravity OAuth` provider preset in the admin UI and select **Sign in with
Antigravity**. The backend starts a PKCE Google OAuth flow, receives the
loopback `/oauth-callback`, and stores the refresh/access tokens without
returning them to the browser. Code Assist project discovery runs when the
account is first used. The public Antigravity client ID is built in and can be
overridden with
`TT_ANTIGRAVITY_OAUTH_CLIENT_ID`. Antigravity OAuth is the only supported
account OAuth mode for Gemini native relay.

The provider account row exposes account-specific model and quota inspection.
The backend reads Antigravity models and per-model reset data from
`fetchAvailableModels`, and reads grouped five-hour and weekly limits from
`retrieveUserQuotaSummary`; permission failures remain visible separately. The
corresponding admin endpoints are
`/admin/api/provider-accounts/{id}/gemini/models` and
`/admin/api/provider-accounts/{id}/gemini/quota`.

Add catalog entries and routes using the exact model IDs returned for the
account. The current Antigravity catalog exposes Gemini 3.1 Pro High through
the callable upstream alias `gemini-pro-agent`, so configure public model
`gemini-3.1-pro-high` with that `upstreamModelId`. Gemini clients can send
native `contents`, `generationConfig`,
`safetySettings`, `tools`, and `toolConfig` bodies to
`/gemini/v1beta/models/{model}:generateContent` or
`/gemini/v1beta/models/{model}:streamGenerateContent`. Client relay
authentication still uses the generated `tokentoxication-...` key, including via
Gemini-style `x-goog-api-key`; that header is not forwarded to Google.

Provider model routes are exact and case-preserving. If a client sends
`MiniMax-M3`, Token Toxication looks up that exact catalog model and rewrites the
forwarded JSON body to the selected route's `upstreamModelId`. Route-level
`stripParams` remove configured top-level request keys, such as `temperature` or
`top_p`, after the model rewrite and before proxying. Nested prompt or message
content is not inspected or persisted for this policy.

Route health is tracked separately from provider account health. A 401 or 403
blocks the provider account. A 429 cools only the selected route for 60 seconds
by default; MiniMax `x-model-quota-remaining` values containing `=0` cool that
selected route for one hour. A 5xx or network failure cools the selected route
for 30 seconds. If the primary route is disabled, blocked, or still cooling
down, the next request uses an eligible backup route. It does not retry another
provider inside the same request.

For Codex with an OpenAI API key, add an OpenAI provider account with base URL
`https://api.openai.com`, Bearer auth, and `openai-responses`. For Codex with
ChatGPT Plus or Pro subscriptions, add one `codex-subscription` provider account
per subscription, use `codex-oauth`, set base URL
`https://chatgpt.com/backend-api`, and paste only the raw refresh token.
Do not paste the full auth JSON file. Codex CLI stores the token at
`~/.codex/auth.json` under `tokens.refresh_token`; extract it with
`jq -r '.tokens.refresh_token' ~/.codex/auth.json`. opencode stores the token at
`~/.local/share/opencode/auth.json` under `openai.refresh`; extract it with
`jq -r '.openai.refresh' ~/.local/share/opencode/auth.json`. The relay forwards
those accounts to `{base_url}/codex/responses` with refreshed ChatGPT OAuth bearer
tokens. Configure routes normally: one subscription can be primary and other
subscriptions can be backups. Configure Codex with a custom provider using base URL
`http://localhost:3000/openai/v1`, wire API `responses`, and the generated
`tokentoxication-...` key. Codex subscription accounts relay Responses requests
to `{base_url}/codex/responses` and query quota from `{base_url}/wham/usage`.
For `codex-oauth` routes, the relay automatically removes `max_output_tokens`
because subscription endpoints follow the Codex CLI request shape. API-key
Responses routes continue forwarding that public API parameter.
The generated opencode config includes only models with eligible OpenAI Chat or
Responses routes and selects the matching AI SDK for each model. When a model
has both route types, the generated config prefers Responses.
Legacy base URLs ending in `/codex` or `/codex/responses` remain supported.
Existing Codex OAuth accounts using those legacy forms are migrated to the account
API root when the database opens, and new account writes are canonicalized.
The provider account row exposes the returned plan, usage windows, model-specific
limits, credits, and reset times through
`/admin/api/provider-accounts/{id}/codex/quota`.

For DeepSeek v4, add a DeepSeek provider account with base URL
`https://api.deepseek.com`, Bearer auth, and `openai-chat`. Use
`deepseek-v4-flash` or `deepseek-v4-pro` as the client model. OpenAI-compatible
clients should use base URL `http://localhost:3000/openai/v1`.

Provider presets mirror opencode IDs where available: `minimax`,
`minimax-coding-plan`, `kimi-for-coding`, `moonshotai`, `zai`,
`zai-coding-plan`, `zhipuai`, `zhipuai-coding-plan`, and `gemini`.
China-region variants are also available for MiniMax and Moonshot.

Request logs store sanitized upstream metadata only: the upstream origin and
path without query parameters, top-level request keys, body byte size, stream
flag, and stripped param names. They do not store prompt/message/input content,
authorization headers, API keys, or raw upstream bodies.

When the provider reports usage, request logs also store input, cached input,
and output token counts. Streaming requests are finalized after the upstream
stream ends so terminal usage events are captured. The admin overview reports
today's cache hit rate as cached input tokens divided by input tokens, and each
request row shows its own rate. Older rows and providers that do not report
cached usage display 0%.

## Configuration

Service configuration is available as CLI flags, environment variables, or
defaults. Run `token-toxication-server --help` for the full flag and env mapping.
Tracing reads the conventional `RUST_LOG` environment variable and falls back to
an info-level server filter. Provider account API keys are stored in the local
SQLite database, so keep `data/` private.

The main listener is always configured with `--bind-addr`. By default it serves
plain HTTP. To make the single server binary terminate TLS itself, set
`--https-mode`:

- `off` serves plain HTTP on `--bind-addr`.
- `cert-files` serves HTTPS on `--bind-addr` from `--tls-cert-path` and
  `--tls-key-path`.
- `acme-http-01` serves HTTPS on `--bind-addr` with app-managed ACME
  certificates and uses `--acme-http-bind-addr` for HTTP-01 challenges.

```bash
token-toxication-server \
  --bind-addr 0.0.0.0:443 \
  --https-mode cert-files \
  --tls-cert-path /etc/token-toxication/fullchain.pem \
  --tls-key-path /etc/token-toxication/privkey.pem
```

```bash
token-toxication-server \
  --bind-addr 0.0.0.0:443 \
  --https-mode acme-http-01 \
  --acme-identifier 91.216.169.227 \
  --acme-email ops@example.com \
  --acme-http-bind-addr 0.0.0.0:80 \
  --acme-cert-dir /var/lib/token-toxication/acme
```

`acme-http-01` accepts either a domain name or a public IP address. IP address
certificates automatically request the ACME `shortlived` profile and are renewed
aggressively because they are valid for only about 160 hours. The ACME account,
certificate chain, private key, and metadata are stored under `--acme-cert-dir`;
they are never stored in SQLite.

HTTP-01 validation expects the challenge listener to be reachable on public port
80. Token Toxication enforces that by default. Use
`--acme-allow-nonstandard-http-port` only for local ACME test servers or when an
explicit public-port forwarding layer maps port 80 to another local port.

When running under systemd as a non-root user on ports 80 or 443, grant only the
low-port bind capability:

```ini
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
```

## OpenAPI and SDK Generation

The backend emits the ignored `openapi/token-toxication.openapi.json` with
`utoipa`.
The ignored admin UI SDK under `apps/admin/src/generated/token-toxication` is
generated with `openapi-nexus`.

```bash
just sdk-generate
```
