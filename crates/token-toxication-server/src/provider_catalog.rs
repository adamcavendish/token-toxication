use crate::models::ProviderPreset;

#[derive(Debug, Clone, Copy)]
struct ProviderPresetSpec {
    id: &'static str,
    label: &'static str,
    name: &'static str,
    provider: &'static str,
    base_url: &'static str,
    auth_mode: &'static str,
    wire_api: &'static str,
    credential_label: &'static str,
    credential_placeholder: &'static str,
    credential_help: &'static str,
    aliases: &'static [&'static str],
}

const API_KEY_HELP: &str = "Paste an upstream API key for this provider account.";
const BEARER_HELP: &str = "Paste an upstream API key. The relay sends it as a Bearer token.";
const ANTIGRAVITY_ACCOUNT_HELP: &str = "Sign in with the Google account used by Antigravity. OAuth tokens are exchanged and stored by the backend. The backend must set TT_ANTIGRAVITY_OAUTH_CLIENT_SECRET.";
const SUBSCRIPTION_HELP: &str = "Paste only the raw refresh token. The CLI stores it at ~/.codex/auth.json as tokens.refresh_token; opencode stores it at ~/.local/share/opencode/auth.json as openai.refresh.";

const PROVIDER_PRESETS: &[ProviderPresetSpec] = &[
    ProviderPresetSpec {
        id: "anthropic",
        label: "Anthropic",
        name: "Anthropic primary",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        auth_mode: "x-api-key",
        wire_api: "anthropic-messages",
        credential_label: "Anthropic API key",
        credential_placeholder: "sk-ant-...",
        credential_help: API_KEY_HELP,
        aliases: &["claude"],
    },
    ProviderPresetSpec {
        id: "openai-responses",
        label: "OpenAI API key",
        name: "OpenAI Responses",
        provider: "openai",
        base_url: "https://api.openai.com",
        auth_mode: "bearer",
        wire_api: "openai-responses",
        credential_label: "OpenAI API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["openai", "gpt"],
    },
    ProviderPresetSpec {
        id: "codex-subscription",
        label: "Codex subscription",
        name: "Codex subscription",
        provider: "codex-subscription",
        base_url: "https://chatgpt.com/backend-api",
        auth_mode: "codex-oauth",
        wire_api: "openai-responses",
        credential_label: "Raw refresh token",
        credential_placeholder: "Paste the value from tokens.refresh_token or openai.refresh",
        credential_help: SUBSCRIPTION_HELP,
        aliases: &[
            "codex",
            "chatgpt",
            "chatgpt-plus",
            "chatgpt-pro",
            "openai-codex",
        ],
    },
    ProviderPresetSpec {
        id: "antigravity",
        label: "Antigravity OAuth",
        name: "Gemini Account",
        provider: "gemini",
        base_url: "https://cloudcode-pa.googleapis.com",
        auth_mode: "antigravity-oauth",
        wire_api: "gemini-generate-content",
        credential_label: "Google account",
        credential_placeholder: "Sign in with Antigravity",
        credential_help: ANTIGRAVITY_ACCOUNT_HELP,
        aliases: &[
            "antigravity",
            "google-antigravity",
            "google",
            "google-ai",
            "google-ai-studio",
            "google-ai-pro",
            "google-ai-ultra",
            "gemini-code-assist",
            "cloudcode",
            "generativelanguage",
        ],
    },
    ProviderPresetSpec {
        id: "deepseek-v4",
        label: "DeepSeek v4",
        name: "DeepSeek v4",
        provider: "deepseek",
        base_url: "https://api.deepseek.com",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "DeepSeek API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &[
            "deepseek",
            "deepseek-v4",
            "deepseek-v4-flash",
            "deepseek-v4-pro",
        ],
    },
    ProviderPresetSpec {
        id: "qwen",
        label: "Qwen",
        name: "Qwen",
        provider: "qwen",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "DashScope API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["dashscope", "aliyun", "qwen3"],
    },
    ProviderPresetSpec {
        id: "minimax",
        label: "MiniMax",
        name: "MiniMax",
        provider: "minimax",
        base_url: "https://api.minimax.io/anthropic/v1",
        auth_mode: "x-api-key",
        wire_api: "anthropic-messages",
        credential_label: "MiniMax API key",
        credential_placeholder: "Bearer token or API key",
        credential_help: API_KEY_HELP,
        aliases: &[],
    },
    ProviderPresetSpec {
        id: "minimax-coding-plan",
        label: "MiniMax Token Plan",
        name: "MiniMax Token Plan",
        provider: "minimax-coding-plan",
        base_url: "https://api.minimax.io/anthropic/v1",
        auth_mode: "x-api-key",
        wire_api: "anthropic-messages",
        credential_label: "MiniMax token-plan key",
        credential_placeholder: "Bearer token or API key",
        credential_help: API_KEY_HELP,
        aliases: &["minimax-token-plan", "minimax-plan"],
    },
    ProviderPresetSpec {
        id: "minimax-cn",
        label: "MiniMax China",
        name: "MiniMax China",
        provider: "minimax-cn",
        base_url: "https://api.minimaxi.com/anthropic/v1",
        auth_mode: "x-api-key",
        wire_api: "anthropic-messages",
        credential_label: "MiniMax China API key",
        credential_placeholder: "Bearer token or API key",
        credential_help: API_KEY_HELP,
        aliases: &[],
    },
    ProviderPresetSpec {
        id: "minimax-cn-coding-plan",
        label: "MiniMax China Token Plan",
        name: "MiniMax China Token Plan",
        provider: "minimax-cn-coding-plan",
        base_url: "https://api.minimaxi.com/anthropic/v1",
        auth_mode: "x-api-key",
        wire_api: "anthropic-messages",
        credential_label: "MiniMax China token-plan key",
        credential_placeholder: "Bearer token or API key",
        credential_help: API_KEY_HELP,
        aliases: &["minimax-cn-token-plan", "minimax-cn-plan"],
    },
    ProviderPresetSpec {
        id: "kimi-for-coding",
        label: "Kimi",
        name: "Kimi",
        provider: "kimi-for-coding",
        base_url: "https://api.kimi.com/coding/v1",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "Kimi API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["kimi"],
    },
    ProviderPresetSpec {
        id: "moonshotai",
        label: "Moonshot AI",
        name: "Moonshot AI",
        provider: "moonshotai",
        base_url: "https://api.moonshot.ai/v1",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "Moonshot API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["moonshot", "moonshot-ai"],
    },
    ProviderPresetSpec {
        id: "moonshotai-cn",
        label: "Moonshot AI China",
        name: "Moonshot AI China",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "Moonshot China API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["moonshot-cn", "moonshot-ai-cn", "kimi-cn"],
    },
    ProviderPresetSpec {
        id: "zai",
        label: "Z.AI",
        name: "Z.AI",
        provider: "zai",
        base_url: "https://api.z.ai/api/paas/v4",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "Z.AI API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["z-ai", "zai"],
    },
    ProviderPresetSpec {
        id: "zai-coding-plan",
        label: "Z.AI Coding Plan",
        name: "Z.AI Coding Plan",
        provider: "zai-coding-plan",
        base_url: "https://api.z.ai/api/coding/paas/v4",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "Z.AI coding-plan key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["z-ai-coding-plan", "zai-coding-plan"],
    },
    ProviderPresetSpec {
        id: "zhipuai",
        label: "Zhipu AI",
        name: "Zhipu AI",
        provider: "zhipuai",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "Zhipu API key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &["zhipu", "zhipu-ai", "zhipuai", "glm", "bigmodel"],
    },
    ProviderPresetSpec {
        id: "zhipuai-coding-plan",
        label: "Zhipu AI Coding Plan",
        name: "Zhipu AI Coding Plan",
        provider: "zhipuai-coding-plan",
        base_url: "https://open.bigmodel.cn/api/coding/paas/v4",
        auth_mode: "bearer",
        wire_api: "openai-chat",
        credential_label: "Zhipu coding-plan key",
        credential_placeholder: "sk-...",
        credential_help: BEARER_HELP,
        aliases: &[
            "zhipu-coding-plan",
            "zhipu-ai-coding-plan",
            "zhipuai-coding-plan",
            "glm-coding-plan",
            "bigmodel-coding-plan",
        ],
    },
];

pub fn provider_presets() -> Vec<ProviderPreset> {
    PROVIDER_PRESETS
        .iter()
        .map(|preset| ProviderPreset {
            id: preset.id.to_string(),
            label: preset.label.to_string(),
            name: preset.name.to_string(),
            provider: preset.provider.to_string(),
            base_url: preset.base_url.to_string(),
            auth_mode: preset.auth_mode.to_string(),
            wire_api: preset.wire_api.to_string(),
            credential_label: preset.credential_label.to_string(),
            credential_placeholder: preset.credential_placeholder.to_string(),
            credential_help: preset.credential_help.to_string(),
            aliases: preset
                .aliases
                .iter()
                .map(|alias| (*alias).to_string())
                .collect(),
        })
        .collect()
}

pub fn normalize_provider_alias(value: &str) -> String {
    let key = provider_alias_key(value);
    if key.is_empty() {
        return "anthropic".to_string();
    }

    for preset in PROVIDER_PRESETS {
        if provider_alias_key(preset.id) == key
            || provider_alias_key(preset.provider) == key
            || preset
                .aliases
                .iter()
                .any(|alias| provider_alias_key(alias) == key)
        {
            return preset.provider.to_string();
        }
    }

    key
}

pub fn default_wire_api_for_provider(provider: &str) -> Option<&'static str> {
    let provider = normalize_provider_alias(provider);
    PROVIDER_PRESETS
        .iter()
        .find(|preset| preset.provider == provider)
        .map(|preset| preset.wire_api)
}

pub fn provider_alias_key(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace([' ', '_'], "-")
        .replace('.', "")
}
