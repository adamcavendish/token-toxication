import { useEffect, useMemo, useState } from "react";
import { ClipboardCopyIcon, DatabaseIcon, KeyRoundIcon } from "lucide-react";

import {
  catalogModelIds,
  copyText,
  enabledCatalogModelOptions,
  opencodeModelOptions,
  preferredCatalogModel,
  routableModelIdsForWireApi,
  shellQuote,
  tomlString,
} from "./helpers";
import { EmptyNotice, Field, SettingRow } from "./shared";
import type { ClientModelOption, OpencodeModelOption } from "./types";
import type { ModelCatalogEntry, RoutableModelCatalogEntry } from "../types";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

export function ClientSetupView({
  models,
  routableModels,
  apiKey,
  setApiKey,
}: {
  models: ModelCatalogEntry[];
  routableModels: RoutableModelCatalogEntry[];
  apiKey: string;
  setApiKey: React.Dispatch<React.SetStateAction<string>>;
}) {
  const serviceOrigin = useMemo(
    () => (typeof window === "undefined" ? "http://127.0.0.1:3000" : window.location.origin),
    [],
  );
  const catalogModels = useMemo(() => catalogModelIds(models), [models]);
  const codexModels = useMemo(
    () => routableModelIdsForWireApi(routableModels, "openai-responses"),
    [routableModels],
  );
  const chatModels = useMemo(
    () => routableModelIdsForWireApi(routableModels, "openai-chat"),
    [routableModels],
  );
  const claudeModels = useMemo(
    () => routableModelIdsForWireApi(routableModels, "anthropic-messages"),
    [routableModels],
  );
  const opencodeModels = useMemo(
    () => opencodeModelOptions(models, routableModels),
    [models, routableModels],
  );
  const piModels = useMemo(() => {
    const responseModels = new Set(codexModels);
    return enabledCatalogModelOptions(models).filter((model) => responseModels.has(model.id));
  }, [codexModels, models]);
  const opencodeModelIds = useMemo(() => opencodeModels.map((model) => model.id), [opencodeModels]);
  const [codexModel, setCodexModel] = useState("");
  const [claudeModel, setClaudeModel] = useState("");
  const [opencodeModel, setOpencodeModel] = useState("");
  const selectedOpencodeModel = opencodeModelIds.includes(opencodeModel)
    ? opencodeModel
    : (opencodeModelIds[0] ?? "");

  useEffect(() => {
    setCodexModel((current) => preferredCatalogModel(current, catalogModels, "gpt-5"));
  }, [catalogModels]);

  useEffect(() => {
    setClaudeModel((current) => preferredCatalogModel(current, catalogModels, "claude-sonnet-4-5"));
  }, [catalogModels]);

  const opencodeWireApi = opencodeModels.find(
    (model) => model.id === selectedOpencodeModel,
  )?.wireApi;

  const snippets = useMemo(
    () =>
      buildClientSetupSnippets({
        apiKey,
        serviceOrigin,
        codexModel,
        claudeModel,
        opencodeModel: selectedOpencodeModel,
        opencodeModels,
        piModels,
      }),
    [
      apiKey,
      serviceOrigin,
      codexModel,
      claudeModel,
      selectedOpencodeModel,
      opencodeModels,
      piModels,
    ],
  );
  const keyLooksValid = apiKey.trim() === "" || apiKey.trim().startsWith("tokentoxication-");

  return (
    <div className="flex flex-col gap-5">
      <Card>
        <CardHeader>
          <CardTitle>Client Setup</CardTitle>
          <CardDescription>
            Generate copy-paste configuration for local AI coding clients.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 lg:grid-cols-[1fr_1fr]">
          <Alert className="lg:col-span-2">
            <KeyRoundIcon className="size-4" />
            <AlertTitle>Use a relay API key secret</AlertTitle>
            <AlertDescription>
              Newly created keys are prefilled here once. Existing rows only show previews, so paste
              the original tokentoxication-* value before copying a setup block.
            </AlertDescription>
          </Alert>
          {!keyLooksValid ? (
            <Alert variant="destructive" className="lg:col-span-2">
              <KeyRoundIcon className="size-4" />
              <AlertTitle>Unexpected key prefix</AlertTitle>
              <AlertDescription>Client keys should start with tokentoxication-.</AlertDescription>
            </Alert>
          ) : null}
          <Field label="Relay API key" htmlFor="setup-api-key">
            <Input
              id="setup-api-key"
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder="tokentoxication-..."
              autoComplete="off"
            />
          </Field>
          <div className="flex flex-col gap-3">
            <SettingRow label="OpenAI base" value={snippets.openaiBaseUrl} />
            <SettingRow label="Anthropic base" value={snippets.anthropicBaseUrl} />
            <div className="grid gap-3 sm:grid-cols-2">
              <SettingRow label="Catalog models" value={String(catalogModels.length)} />
              <SettingRow label="Chat routed" value={String(chatModels.length)} />
              <SettingRow label="Responses routed" value={String(codexModels.length)} />
              <SettingRow label="Messages routed" value={String(claudeModels.length)} />
            </div>
          </div>
          {catalogModels.length === 0 ? (
            <Alert className="lg:col-span-2">
              <DatabaseIcon className="size-4" />
              <AlertTitle>No catalog models yet</AlertTitle>
              <AlertDescription>
                Add exact model names in Model Catalog, then bind them to provider routes. Client
                setup will populate from that catalog.
              </AlertDescription>
            </Alert>
          ) : (
            <div className="grid gap-4 lg:col-span-2 lg:grid-cols-3">
              <ClientModelField
                id="setup-codex-model"
                label="Codex"
                value={codexModel}
                onChange={setCodexModel}
                options={catalogModels}
                routedOptions={codexModels}
                routeLabel="Responses"
              />
              <ClientModelField
                id="setup-claude-model"
                label="Claude Code"
                value={claudeModel}
                onChange={setClaudeModel}
                options={catalogModels}
                routedOptions={claudeModels}
                routeLabel="Messages"
              />
              <ClientModelField
                id="setup-opencode-model"
                label="opencode"
                value={selectedOpencodeModel}
                onChange={setOpencodeModel}
                options={opencodeModelIds}
                routedOptions={opencodeModelIds}
                routeLabel="Chat or Responses"
              />
            </div>
          )}
        </CardContent>
      </Card>

      {catalogModels.length > 0 ? (
        <Tabs defaultValue="codex">
          <TabsList>
            <TabsTrigger value="codex">Codex</TabsTrigger>
            <TabsTrigger value="claude">Claude Code</TabsTrigger>
            <TabsTrigger value="opencode">opencode</TabsTrigger>
            <TabsTrigger value="pi">Pi</TabsTrigger>
          </TabsList>
          <TabsContent value="codex">
            <ClientSnippetCard
              title="Codex profile"
              description="Writes a dedicated profile using the Responses wire API."
              endpoint="/openai/v1/responses"
              model={codexModel}
              snippet={snippets.codex}
            />
          </TabsContent>
          <TabsContent value="claude">
            <ClientSnippetCard
              title="Claude Code environment"
              description="Points Claude Code at the Anthropic Messages namespace."
              endpoint="/anthropic/v1/messages"
              model={claudeModel}
              snippet={snippets.claudeCode}
            />
          </TabsContent>
          <TabsContent value="opencode">
            {opencodeModels.length > 0 ? (
              <ClientSnippetCard
                title="opencode project config"
                description="Binds each model to the AI SDK matching its configured OpenAI route."
                endpoint={
                  opencodeWireApi === "openai-responses"
                    ? "/openai/v1/responses"
                    : "/openai/v1/chat/completions"
                }
                model={selectedOpencodeModel}
                snippet={snippets.opencode}
              />
            ) : (
              <EmptyNotice
                title="No opencode routes"
                body="Add an eligible OpenAI Chat or Responses route to generate an opencode config."
              />
            )}
          </TabsContent>
          <TabsContent value="pi">
            {piModels.length > 0 ? (
              <ClientSnippetCard
                title="Pi custom provider"
                description="Writes a complete Pi models.json file using the OpenAI Responses API. Back up any existing Pi configuration first."
                endpoint="/openai/v1/responses"
                model={`${piModels.length} routed model${piModels.length === 1 ? "" : "s"}`}
                snippet={snippets.pi}
              />
            ) : (
              <EmptyNotice
                title="No Pi routes"
                body="Add an eligible OpenAI Responses route to generate a Pi provider config."
              />
            )}
          </TabsContent>
        </Tabs>
      ) : null}
    </div>
  );
}

function ClientModelField({
  id,
  label,
  value,
  onChange,
  options,
  routedOptions,
  routeLabel,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: string[];
  routedOptions: string[];
  routeLabel: string;
}) {
  const isRouted = routedOptions.includes(value);
  return (
    <div className="flex flex-col gap-3 rounded-md border p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor={id}>{label}</Label>
          <span className="text-xs text-muted-foreground">{routeLabel} route required</span>
        </div>
        <Badge variant={isRouted ? "secondary" : "outline"}>
          {isRouted ? "routed" : "not routed"}
        </Badge>
      </div>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger id={id}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {options.map((model) => (
              <SelectItem key={model} value={model}>
                {model}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>
  );
}

function ClientSnippetCard({
  title,
  description,
  endpoint,
  model,
  snippet,
}: {
  title: string;
  description: string;
  endpoint: string;
  model: string;
  snippet: string;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
        <CardAction>
          <Button type="button" onClick={() => copyText(snippet)}>
            <ClipboardCopyIcon data-icon="inline-start" />
            Copy
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="grid gap-3 md:grid-cols-2">
          <SettingRow label="Route" value={endpoint} />
          <SettingRow label="Model" value={model || "not set"} />
        </div>
        <pre className="max-h-[560px] overflow-auto rounded-md border bg-muted/40 p-3 text-xs leading-5">
          <code>{snippet}</code>
        </pre>
      </CardContent>
    </Card>
  );
}

function buildClientSetupSnippets({
  apiKey,
  serviceOrigin,
  codexModel,
  claudeModel,
  opencodeModel,
  opencodeModels,
  piModels,
}: {
  apiKey: string;
  serviceOrigin: string;
  codexModel: string;
  claudeModel: string;
  opencodeModel: string;
  opencodeModels: OpencodeModelOption[];
  piModels: ClientModelOption[];
}) {
  const origin = serviceOrigin.replace(/\/+$/, "");
  const relayApiKey = apiKey.trim() || "tokentoxication-REPLACE_ME";
  const openaiBaseUrl = `${origin}/openai/v1`;
  const anthropicBaseUrl = `${origin}/anthropic`;
  const codexModelName = codexModel.trim() || "gpt-5";
  const claudeModelName = claudeModel.trim() || "claude-sonnet-4-5";
  const opencodeModelName = opencodeModel.trim() || opencodeModels[0]?.id || "";
  const opencodeProvider = "token-toxication";
  const opencodeConfig = JSON.stringify(
    {
      $schema: "https://opencode.ai/config.json",
      provider: {
        [opencodeProvider]: {
          name: "Token Toxication",
          options: {
            baseURL: openaiBaseUrl,
            apiKey: "{env:TOKEN_TOXICATION_API_KEY}",
          },
          models: Object.fromEntries(
            opencodeModels.map((model) => [
              model.id,
              {
                name: model.displayName,
                provider: {
                  npm:
                    model.wireApi === "openai-responses"
                      ? "@ai-sdk/openai"
                      : "@ai-sdk/openai-compatible",
                },
              },
            ]),
          ),
        },
      },
      ...(opencodeModelName
        ? {
            model: `${opencodeProvider}/${opencodeModelName}`,
            small_model: `${opencodeProvider}/${opencodeModelName}`,
          }
        : {}),
    },
    null,
    2,
  );
  const piModelsConfig = JSON.stringify(
    {
      providers: {
        "token-toxication": {
          name: "Token Toxication",
          baseUrl: openaiBaseUrl,
          api: "openai-responses",
          apiKey: "$TOKEN_TOXICATION_API_KEY",
          models: piModels.map((model) => ({
            id: model.id,
            name: model.displayName,
            reasoning: true,
          })),
        },
      },
    },
    null,
    2,
  );

  return {
    openaiBaseUrl,
    anthropicBaseUrl,
    codex: [
      `export TOKEN_TOXICATION_API_KEY=${shellQuote(relayApiKey)}`,
      "mkdir -p ~/.codex",
      "cat > ~/.codex/token-toxication.config.toml <<'TOML'",
      `model = ${tomlString(codexModelName)}`,
      `model_provider = ${tomlString("token-toxication")}`,
      "",
      "[model_providers.token-toxication]",
      `name = ${tomlString("Token Toxication")}`,
      `base_url = ${tomlString(openaiBaseUrl)}`,
      `env_key = ${tomlString("TOKEN_TOXICATION_API_KEY")}`,
      `wire_api = ${tomlString("responses")}`,
      "TOML",
      "",
      "codex --profile token-toxication",
    ].join("\n"),
    claudeCode: [
      `export ANTHROPIC_BASE_URL=${shellQuote(anthropicBaseUrl)}`,
      `export ANTHROPIC_AUTH_TOKEN=${shellQuote(relayApiKey)}`,
      `export ANTHROPIC_MODEL=${shellQuote(claudeModelName)}`,
      "export CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1",
      "",
      `claude -p ${shellQuote("Reply with one word: connected")}`,
    ].join("\n"),
    opencode: [
      `export TOKEN_TOXICATION_API_KEY=${shellQuote(relayApiKey)}`,
      "cat > opencode.json <<'JSON'",
      opencodeConfig,
      "JSON",
      "",
      "opencode",
    ].join("\n"),
    pi: [
      `export TOKEN_TOXICATION_API_KEY=${shellQuote(relayApiKey)}`,
      "mkdir -p ~/.pi/agent",
      "cat > ~/.pi/agent/models.json <<'JSON'",
      piModelsConfig,
      "JSON",
      "pi",
    ].join("\n"),
  };
}
