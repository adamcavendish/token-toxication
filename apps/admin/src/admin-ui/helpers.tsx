import type { Dispatch, SetStateAction } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";

import type {
  CodexAccountQuotaResponse,
  GeminiAccountQuotaResponse,
  ModelCatalogEntry,
  ProviderAccount,
  ProviderModelRoute,
  ProviderPreset,
  RequestLog,
  RoutableModelCatalogEntry,
} from "../types";
import type { ClientModelOption, CreateAccountForm, OpencodeModelOption } from "./types";

export function statusBadge(status: string, active: boolean) {
  if (!active) {
    return <Badge variant="outline">paused</Badge>;
  }
  if (status === "healthy") {
    return <Badge variant="secondary">healthy</Badge>;
  }
  if (status === "blocked") {
    return <Badge variant="destructive">blocked</Badge>;
  }
  return <Badge variant="outline">{status}</Badge>;
}

export function routeRoleBadge(role: string) {
  if (role === "primary") {
    return <Badge variant="secondary">primary</Badge>;
  }
  return <Badge variant="outline">{role}</Badge>;
}

export function routeCountForAccount(routes: ProviderModelRoute[], accountId: string) {
  return routes.filter((route) => route.providerAccountId === accountId).length;
}

export function routeCountForModel(routes: ProviderModelRoute[], modelId: string) {
  return routes.filter((route) => route.publicModelId === modelId).length;
}

export function accountName(accounts: readonly ProviderAccount[], accountId: string) {
  return accounts.find((account) => account.id === accountId)?.name ?? accountId;
}

export function uniqueSorted(values: string[]) {
  return Array.from(new Set(values)).sort((left, right) => left.localeCompare(right));
}

export function commaSeparatedValues(value: string) {
  return uniqueSorted(
    value
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean),
  );
}

export function formatRoutePolicy(route: ProviderModelRoute) {
  if (route.stripParams.length === 0) {
    return "No stripped params";
  }
  return `strip ${route.stripParams.join(", ")}`;
}

export function statusCodeBadge(status: number) {
  if (status >= 200 && status < 300) {
    return <Badge variant="secondary">{status}</Badge>;
  }
  if (status === 429 || status >= 500) {
    return <Badge variant="destructive">{status}</Badge>;
  }
  return <Badge variant="outline">{status}</Badge>;
}

export function formatLogModel(log: RequestLog) {
  const publicModel = log.model || "unknown";
  if (!log.upstreamModel || log.upstreamModel === publicModel) {
    return publicModel;
  }
  return `${publicModel} -> ${log.upstreamModel}`;
}

export function formatRequestSummary(log: RequestLog) {
  const summary = log.requestSummary;
  if (!summary) {
    return "No request summary";
  }
  const stream = summary.stream ? "stream" : "non-stream";
  const keys = summary.topLevelKeys.length > 0 ? summary.topLevelKeys.join(", ") : "no keys";
  const stripped =
    summary.strippedParams.length > 0 ? ` · stripped ${summary.strippedParams.join(", ")}` : "";
  return `${formatNumber(summary.bodyBytes)} bytes · ${stream} · keys ${keys}${stripped}`;
}

export function providerPresetForForm(form: CreateAccountForm, presets: ProviderPreset[]) {
  return presets.find(
    (preset) =>
      preset.provider === form.provider &&
      preset.baseUrl === form.baseUrl &&
      preset.authMode === form.authMode &&
      preset.wireApi === form.wireApi,
  );
}

export function accountPresetValue(form: CreateAccountForm, presets: ProviderPreset[]) {
  return providerPresetForForm(form, presets)?.id ?? "custom";
}

export function applyAccountPreset(
  presetId: string,
  presets: ProviderPreset[],
  setForm: Dispatch<SetStateAction<CreateAccountForm>>,
) {
  const preset = presets.find((item) => item.id === presetId);
  if (!preset) {
    return;
  }
  setForm((current) => ({
    ...current,
    name: current.name || preset.name,
    provider: preset.provider,
    baseUrl: preset.baseUrl,
    authMode: preset.authMode,
    wireApi: preset.wireApi,
  }));
}

export function routableModelIdsForWireApi(models: RoutableModelCatalogEntry[], wireApi: string) {
  return uniqueSorted(models.filter((model) => model.wireApi === wireApi).map((model) => model.id));
}

export function catalogModelIds(models: ModelCatalogEntry[]) {
  return uniqueSorted(models.filter((model) => model.enabled).map((model) => model.id));
}

export function enabledCatalogModelOptions(models: ModelCatalogEntry[]): ClientModelOption[] {
  return models
    .filter((model) => model.enabled && model.id)
    .map((model) => ({
      id: model.id,
      displayName: model.displayName || model.id,
    }))
    .sort((left, right) => left.id.localeCompare(right.id));
}

export function opencodeModelOptions(
  models: ModelCatalogEntry[],
  routableModels: RoutableModelCatalogEntry[],
): OpencodeModelOption[] {
  const chatModels = new Set(routableModelIdsForWireApi(routableModels, "openai-chat"));
  const responseModels = new Set(routableModelIdsForWireApi(routableModels, "openai-responses"));

  return enabledCatalogModelOptions(models)
    .filter((model) => chatModels.has(model.id) || responseModels.has(model.id))
    .map((model) => ({
      ...model,
      wireApi: responseModels.has(model.id) ? "openai-responses" : "openai-chat",
    }));
}

export function preferredCatalogModel(current: string, catalogModels: string[], fallback: string) {
  if (catalogModels.length === 0) {
    return current || fallback;
  }
  return current && catalogModels.includes(current) ? current : catalogModels[0];
}

export function shellQuote(value: string) {
  return `'${value.replace(/'/g, "'\\''")}'`;
}

export function tomlString(value: string) {
  return JSON.stringify(value);
}

export function wireApiLabel(value: string) {
  switch (value) {
    case "openai-chat":
      return "OpenAI Chat";
    case "openai-responses":
      return "OpenAI Responses";
    case "anthropic-messages":
      return "Anthropic Messages";
    case "gemini-generate-content":
      return "Gemini GenerateContent";
    default:
      return value;
  }
}

export function upstreamPathForWireApi(value: string, authMode?: string) {
  if (isCodexSubscriptionAuth(authMode ?? "")) {
    return "/codex/responses";
  }
  if (isAntigravityAccountAuth(authMode ?? "")) {
    return "/v1internal:generateContent";
  }
  switch (value) {
    case "openai-chat":
      return "/chat/completions";
    case "openai-responses":
      return "/v1/responses";
    case "gemini-generate-content":
      return "/v1beta/models/{model}:generateContent";
    default:
      return "/v1/messages";
  }
}

export function isCodexSubscriptionAuth(value: string) {
  return value === "codex-oauth";
}

export function isAntigravityAccountAuth(value: string) {
  return value === "antigravity-oauth";
}

export function isGeminiAccount(account: ProviderAccount) {
  return account.provider === "gemini" && isAntigravityAccountAuth(account.authMode);
}

export function isCodexAccount(account: ProviderAccount) {
  return isCodexSubscriptionAuth(account.authMode);
}

export function numberFromInput(value: string) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

export function formatNumber(value: number) {
  return new Intl.NumberFormat().format(value);
}

export function formatCacheHitRate(cachedInputTokens: number, inputTokens: number) {
  const rate = inputTokens > 0 ? cachedInputTokens / inputTokens : 0;
  return new Intl.NumberFormat(undefined, {
    style: "percent",
    maximumFractionDigits: 1,
  }).format(rate);
}

export function niceChartMaximum(value: number) {
  if (value <= 4) {
    return 4;
  }
  const magnitude = 10 ** Math.floor(Math.log10(value));
  const normalized = value / magnitude;
  const factor = normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
  return factor * magnitude;
}

export function formatChartTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function formatChartInterval(startedAt: string, durationSeconds: number) {
  const endedAt = new Date(new Date(startedAt).getTime() + durationSeconds * 1_000);
  return `${formatChartTime(startedAt)}–${formatChartTime(endedAt.toISOString())}`;
}

export function formatRequestCount(value: number) {
  return `${formatNumber(value)} ${value === 1 ? "request" : "requests"}`;
}

export function formatGeminiTier(tier: GeminiAccountQuotaResponse["currentTier"]) {
  if (!tier) {
    return "unknown";
  }
  return tier.name || tier.id || "unknown";
}

export function quotaPercent(value: number | null | undefined) {
  return value == null ? undefined : Math.max(0, Math.min(100, value * 100));
}

export function formatQuotaPercent(value: number) {
  if (value === 100) {
    return "100%";
  }
  return `${value.toFixed(value >= 99 ? 3 : 1)}%`;
}

export function codexUsedPercent(value: number | null | undefined) {
  return value == null || !Number.isFinite(value) ? undefined : Math.max(0, Math.min(100, value));
}

export function formatCodexWindow(seconds: number | null | undefined) {
  if (seconds == null || !Number.isFinite(seconds) || seconds <= 0) {
    return "unknown";
  }
  if (seconds >= 86_400) {
    const days = Math.round(seconds / 86_400);
    if (Math.abs(seconds - days * 86_400) <= 120) {
      return `${days} ${days === 1 ? "day" : "days"}`;
    }
    return `${(seconds / 86_400).toFixed(1)} days`;
  }
  if (seconds >= 3_600) {
    const hours = Math.round(seconds / 3_600);
    if (Math.abs(seconds - hours * 3_600) <= 60) {
      return `${hours} ${hours === 1 ? "hour" : "hours"}`;
    }
    return `${(seconds / 3_600).toFixed(1)} hours`;
  }
  if (seconds >= 60) {
    const minutes = Math.round(seconds / 60);
    return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
  }
  return `${seconds} ${seconds === 1 ? "second" : "seconds"}`;
}

export function formatQuotaReset(value: string | null | undefined) {
  return value ? formatDate(value) : "unknown";
}

export function codexQuotaStatus(
  allowed: boolean | null | undefined,
  limitReached: boolean | null | undefined,
) {
  if (limitReached === true || allowed === false) {
    return <Badge variant="destructive">limited</Badge>;
  }
  if (allowed === true) {
    return <Badge variant="secondary">available</Badge>;
  }
  return <Badge variant="outline">unknown</Badge>;
}

export function formatCodexCredits(credits: NonNullable<CodexAccountQuotaResponse["credits"]>) {
  if (credits.unlimited) {
    return "Unlimited";
  }
  if (credits.balance) {
    return `Balance ${credits.balance}`;
  }
  if (credits.hasCredits === true) {
    return "Available";
  }
  if (credits.hasCredits === false) {
    return "None";
  }
  return "unknown";
}

export function formatCodexSpendControl(
  reached: boolean | null | undefined,
  limit:
    | NonNullable<NonNullable<CodexAccountQuotaResponse["spendControl"]>["individualLimit"]>
    | null
    | undefined,
) {
  if (!limit) {
    return reached ? "Reached" : "unknown";
  }
  const amount =
    limit.used && limit.limit
      ? `${limit.used} / ${limit.limit}`
      : limit.remaining
        ? `${limit.remaining} remaining`
        : "Configured";
  return reached ? `${amount}, reached` : amount;
}

export function formatOptionalNumber(value: number | null | undefined) {
  return value == null ? "unknown" : formatNumber(value);
}

export function humanizeIdentifier(value: string) {
  const text = value.replaceAll("_", " ").trim();
  return text ? `${text.charAt(0).toUpperCase()}${text.slice(1)}` : "Unknown";
}

export function formatDate(value: string | null | undefined) {
  if (!value) {
    return "never";
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export async function copyText(value: string) {
  await navigator.clipboard.writeText(value);
  toast.success("Copied");
}
