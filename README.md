# Token Toxication

Token Toxication is a Rust + React relay service. It provides self-hosted
Anthropic Messages, OpenAI Responses, and OpenAI Chat relay endpoints,
admin-managed API keys, provider account rotation, request logging, and a Vite+
shadcn dashboard.

## Layout

- `crates/token-toxication-server` - Axum backend and SQLite persistence.
- `apps/admin` - React + Vite+ + shadcn admin UI.
- `.github/workflows/ci.yml` - Rust and Vite+ CI.

## Quick Start

```bash
cp .env.example .env
cd apps/admin && vp install && vp build
cd ../..
cargo run -p token-toxication-server --bin token-toxication-server
```

Open `http://localhost:3000` and sign in with `TT_ADMIN_USERNAME` /
`TT_ADMIN_PASSWORD`. Add at least one provider account before using the relay.

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

Model discovery uses the same relay API key authentication. Concrete model names
come from active provider accounts with a non-empty `model_hint`:

```bash
curl http://localhost:3000/openai/v1/models \
  -H "Authorization: Bearer tokentoxication-..."
```

## Provider Accounts

Each provider account has a `wire_api` protocol:

- `anthropic-messages` forwards to `{base_url}/v1/messages`.
- `openai-chat` forwards to `{base_url}/chat/completions`.
- `openai-responses` forwards to `{base_url}/v1/responses`.

For Anthropic-compatible clients, use base URL `http://localhost:3000/anthropic`.

There is no global upstream base URL. Each provider account owns its base URL,
auth mode, wire protocol, priority, and optional `model_hint`. For
OpenAI-compatible chat providers such as Qwen, Kimi, GLM, and DeepSeek, create
accounts with `openai-chat` and model hints such as `qwen`, `kimi`, `glm`, or
`deepseek`; clients can keep using `/openai/v1/chat/completions` and switch only
the model name. Use exact model names as `model_hint` values when clients need
`/openai/v1/models` or `/anthropic/v1/models` discovery.

For Codex, add an OpenAI provider account with base URL
`https://api.openai.com`, Bearer auth, and `openai-responses`. Configure Codex
with a custom provider using base URL `http://localhost:3000/openai/v1`, wire API
`responses`, and the generated `tokentoxication-...` key.

For DeepSeek v4, add a DeepSeek provider account with base URL
`https://api.deepseek.com`, Bearer auth, and `openai-chat`. Use
`deepseek-v4-flash` or `deepseek-v4-pro` as the client model. OpenAI-compatible
clients should use base URL `http://localhost:3000/openai/v1`.

## Configuration

Service configuration is available as CLI flags, environment variables, or
defaults. Run `token-toxication-server --help` for the full flag and env mapping.
Tracing reads the conventional `RUST_LOG` environment variable and falls back to
an info-level server filter. Provider account API keys are stored in the local
SQLite database, so keep `data/` private.

## OpenAPI and SDK Generation

The backend emits the ignored `openapi/token-toxication.openapi.json` with
`utoipa`.
The admin UI SDK under `apps/admin/src/generated/token-toxication` is generated
with `openapi-nexus`.
