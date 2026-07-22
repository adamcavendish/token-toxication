import type { CodexAccountQuotaWindow, ProviderAccount } from "../types";

export type CreateKeyForm = {
  name: string;
  description: string;
  permissions: string;
  rateLimitPerMinute: string;
  concurrencyLimit: string;
  dailyCostLimit: string;
};

export type CreateAccountForm = {
  name: string;
  provider: string;
  baseUrl: string;
  authMode: string;
  wireApi: string;
  apiKey: string;
  isActive: boolean;
  priority: string;
};

export type ModelCatalogForm = {
  id: string;
  displayName: string;
  family: string;
  enabled: boolean;
};

export type ProviderRouteForm = {
  publicModelId: string;
  providerAccountId: string;
  upstreamModelId: string;
  wireApi: string;
  role: string;
  enabled: boolean;
  stripParams: string;
};

export type ClientModelOption = {
  id: string;
  displayName: string;
};

export type OpencodeWireApi = "openai-chat" | "openai-responses";

export type OpencodeModelOption = ClientModelOption & {
  wireApi: OpencodeWireApi;
};

export type CodexQuotaRow = {
  limitId: string;
  displayName: string;
  windowName: string;
  window: CodexAccountQuotaWindow | null;
  allowed?: boolean | null;
  limitReached?: boolean | null;
};

export type AntigravityOAuthMessage = {
  type: "token-toxication:antigravity-oauth";
  success: boolean;
  accountId?: string;
  error?: string;
};

export const emptyKeyForm: CreateKeyForm = {
  name: "",
  description: "",
  permissions: "all",
  rateLimitPerMinute: "0",
  concurrencyLimit: "0",
  dailyCostLimit: "0",
};

export const emptyAccountForm: CreateAccountForm = {
  name: "",
  provider: "anthropic",
  baseUrl: "https://api.anthropic.com",
  authMode: "x-api-key",
  wireApi: "anthropic-messages",
  apiKey: "",
  isActive: true,
  priority: "0",
};

export function accountFormFromAccount(account: ProviderAccount): CreateAccountForm {
  return {
    name: account.name,
    provider: account.provider,
    baseUrl: account.baseUrl,
    authMode: account.authMode,
    wireApi: account.wireApi,
    apiKey: "",
    isActive: account.isActive,
    priority: String(account.priority),
  };
}

export const emptyModelForm: ModelCatalogForm = {
  id: "",
  displayName: "",
  family: "other",
  enabled: true,
};

export const emptyRouteForm: ProviderRouteForm = {
  publicModelId: "",
  providerAccountId: "",
  upstreamModelId: "",
  wireApi: "openai-chat",
  role: "primary",
  enabled: true,
  stripParams: "",
};
