import { useCallback, useEffect, useState } from "react";
import type React from "react";
import { toast } from "sonner";

import { api, clearStoredToken, getStoredToken, setStoredToken } from "../api";
import {
  commaSeparatedValues,
  isAntigravityAccountAuth,
  numberFromInput,
  routeCountForAccount,
} from "./helpers";
import {
  accountFormFromAccount,
  emptyAccountForm,
  emptyKeyForm,
  emptyModelForm,
  emptyRouteForm,
  type AntigravityOAuthMessage,
  type CreateAccountForm,
  type CreateKeyForm,
  type ModelCatalogForm,
  type ProviderRouteForm,
} from "./types";
import type {
  ApiKey,
  CodexAccountQuotaResponse,
  Dashboard,
  GeminiAccountModelsResponse,
  GeminiAccountQuotaResponse,
  ModelCatalogEntry,
  ProviderAccount,
  ProviderModelRoute,
  ProviderPreset,
  RequestLog,
  RoutableModelCatalogEntry,
} from "../types";

export function useAdminWorkspace() {
  const [token, setToken] = useState(() => getStoredToken());
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [accounts, setAccounts] = useState<ProviderAccount[]>([]);
  const [providerPresets, setProviderPresets] = useState<ProviderPreset[]>([]);
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogEntry[]>([]);
  const [routableModels, setRoutableModels] = useState<RoutableModelCatalogEntry[]>([]);
  const [modelRoutes, setModelRoutes] = useState<ProviderModelRoute[]>([]);
  const [logs, setLogs] = useState<RequestLog[]>([]);
  const [isLoading, setIsLoading] = useState(Boolean(token));
  const [isKeySheetOpen, setIsKeySheetOpen] = useState(false);
  const [isAccountSheetOpen, setIsAccountSheetOpen] = useState(false);
  const [editingAccount, setEditingAccount] = useState<ProviderAccount | null>(null);
  const [isModelSheetOpen, setIsModelSheetOpen] = useState(false);
  const [isRouteSheetOpen, setIsRouteSheetOpen] = useState(false);
  const [createKeyForm, setCreateKeyForm] = useState<CreateKeyForm>(emptyKeyForm);
  const [createAccountForm, setCreateAccountForm] = useState<CreateAccountForm>(emptyAccountForm);
  const [createModelForm, setCreateModelForm] = useState<ModelCatalogForm>(emptyModelForm);
  const [createRouteForm, setCreateRouteForm] = useState<ProviderRouteForm>(emptyRouteForm);
  const [createdSecret, setCreatedSecret] = useState<string | null>(null);
  const [clientSetupApiKey, setClientSetupApiKey] = useState("");
  const [geminiDetailsAccount, setGeminiDetailsAccount] = useState<ProviderAccount | null>(null);
  const [geminiModels, setGeminiModels] = useState<GeminiAccountModelsResponse | null>(null);
  const [geminiQuota, setGeminiQuota] = useState<GeminiAccountQuotaResponse | null>(null);
  const [isGeminiDetailsLoading, setIsGeminiDetailsLoading] = useState(false);
  const [geminiDetailsError, setGeminiDetailsError] = useState<string | null>(null);
  const [codexDetailsAccount, setCodexDetailsAccount] = useState<ProviderAccount | null>(null);
  const [codexQuota, setCodexQuota] = useState<CodexAccountQuotaResponse | null>(null);
  const [isCodexDetailsLoading, setIsCodexDetailsLoading] = useState(false);
  const [codexDetailsError, setCodexDetailsError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!getStoredToken()) {
      return;
    }
    setIsLoading(true);
    try {
      const [
        nextDashboard,
        nextKeys,
        nextAccounts,
        nextPresets,
        nextCatalog,
        nextRoutableModels,
        nextRoutes,
        nextLogs,
      ] = await Promise.all([
        api.dashboard(),
        api.apiKeys(),
        api.providerAccounts(),
        api.providerPresets(),
        api.modelCatalog(),
        api.routableModelCatalog(),
        api.providerModelRoutes(),
        api.requestLogs(50),
      ]);
      setDashboard(nextDashboard);
      setApiKeys(nextKeys);
      setAccounts(nextAccounts);
      setProviderPresets(nextPresets);
      setModelCatalog(nextCatalog);
      setRoutableModels(nextRoutableModels);
      setModelRoutes(nextRoutes);
      setLogs(nextLogs);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to load dashboard");
      if (error instanceof Error && /token|session|credentials/i.test(error.message)) {
        clearStoredToken();
        setToken(null);
      }
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh, token]);

  useEffect(() => {
    function handleOAuthMessage(event: MessageEvent<AntigravityOAuthMessage>) {
      if (
        event.origin !== window.location.origin ||
        event.data?.type !== "token-toxication:antigravity-oauth"
      ) {
        return;
      }
      if (event.data.success) {
        setEditingAccount(null);
        setCreateAccountForm(emptyAccountForm);
        setIsAccountSheetOpen(false);
        toast.success("Antigravity account connected");
        void refresh();
      } else {
        toast.error(event.data.error || "Antigravity sign-in failed");
      }
    }

    window.addEventListener("message", handleOAuthMessage);
    return () => window.removeEventListener("message", handleOAuthMessage);
  }, [refresh]);

  async function handleLogin(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const response = await api.login(username, password);
      setStoredToken(response.token);
      setToken(response.token);
      setPassword("");
      toast.success("Signed in");
      await refresh();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Sign-in failed");
    }
  }

  async function handleLogout() {
    try {
      await api.logout();
    } catch {
      // Local logout still clears the session token.
    }
    clearStoredToken();
    setToken(null);
    setDashboard(null);
    toast.success("Signed out");
  }

  async function handleCreateKey(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const response = await api.createApiKey({
      name: createKeyForm.name,
      description: createKeyForm.description,
      permissions:
        createKeyForm.permissions === "all"
          ? []
          : createKeyForm.permissions.split(",").map((item) => item.trim()),
      rateLimitPerMinute: numberFromInput(createKeyForm.rateLimitPerMinute),
      concurrencyLimit: numberFromInput(createKeyForm.concurrencyLimit),
      dailyCostLimit: Number(createKeyForm.dailyCostLimit) || 0,
    });
    setCreatedSecret(response.secret);
    setClientSetupApiKey(response.secret);
    setCreateKeyForm(emptyKeyForm);
    setIsKeySheetOpen(false);
    toast.success("API key created");
    await refresh();
  }

  function openCreateAccount() {
    setEditingAccount(null);
    setCreateAccountForm(emptyAccountForm);
    setIsAccountSheetOpen(true);
  }

  function openEditAccount(account: ProviderAccount) {
    setEditingAccount(account);
    setCreateAccountForm(accountFormFromAccount(account));
    setIsAccountSheetOpen(true);
  }

  function closeAccountSheet() {
    setEditingAccount(null);
    setCreateAccountForm(emptyAccountForm);
    setIsAccountSheetOpen(false);
  }

  function handleAccountSheetOpenChange(open: boolean) {
    if (open) {
      setIsAccountSheetOpen(true);
    } else {
      closeAccountSheet();
    }
  }

  async function handleSaveAccount(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingAccount && isAntigravityAccountAuth(createAccountForm.authMode)) {
      await launchAntigravityOAuth({
        name: createAccountForm.name,
        priority: numberFromInput(createAccountForm.priority),
      });
      return;
    }
    if (editingAccount) {
      await api.updateProviderAccount(editingAccount.id, {
        name: createAccountForm.name,
        provider: createAccountForm.provider,
        baseUrl: createAccountForm.baseUrl,
        authMode: createAccountForm.authMode,
        wireApi: createAccountForm.wireApi,
        apiKey: createAccountForm.apiKey.trim() || undefined,
        isActive: createAccountForm.isActive,
        priority: numberFromInput(createAccountForm.priority),
      });
      toast.success("Provider account updated");
    } else {
      await api.createProviderAccount({
        name: createAccountForm.name,
        provider: createAccountForm.provider,
        baseUrl: createAccountForm.baseUrl,
        authMode: createAccountForm.authMode,
        wireApi: createAccountForm.wireApi,
        apiKey: createAccountForm.apiKey,
        isActive: createAccountForm.isActive,
        priority: numberFromInput(createAccountForm.priority),
      });
      toast.success("Provider account created");
    }
    closeAccountSheet();
    await refresh();
  }

  async function launchAntigravityOAuth({
    accountId,
    name,
    priority,
  }: {
    accountId?: string;
    name: string;
    priority: number;
  }) {
    const popup = window.open(
      "about:blank",
      "token-toxication-antigravity-oauth",
      "popup,width=560,height=720",
    );
    if (!popup) {
      toast.error("Allow popups to sign in with Antigravity");
      return;
    }
    try {
      const response = await api.startAntigravityOAuth({
        accountId,
        name,
        priority,
        redirectUri: `${window.location.origin}/oauth-callback`,
      });
      popup.location.replace(response.authorizationUrl);
    } catch (error) {
      popup.close();
      toast.error(error instanceof Error ? error.message : "Unable to start Antigravity sign-in");
    }
  }

  async function reconnectAntigravityAccount(account: ProviderAccount) {
    await launchAntigravityOAuth({
      accountId: account.id,
      name: account.name,
      priority: account.priority,
    });
  }

  async function inspectGeminiAccount(account: ProviderAccount) {
    setGeminiDetailsAccount(account);
    setGeminiModels(null);
    setGeminiQuota(null);
    setGeminiDetailsError(null);
    setIsGeminiDetailsLoading(true);
    try {
      const [models, quota] = await Promise.all([
        api.geminiAccountModels(account.id),
        api.geminiAccountQuota(account.id),
      ]);
      setGeminiModels(models);
      setGeminiQuota(quota);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unable to load Gemini account data";
      setGeminiDetailsError(message);
      toast.error(message);
    } finally {
      setIsGeminiDetailsLoading(false);
    }
  }

  async function inspectCodexAccount(account: ProviderAccount) {
    setCodexDetailsAccount(account);
    setCodexQuota(null);
    setCodexDetailsError(null);
    setIsCodexDetailsLoading(true);
    try {
      setCodexQuota(await api.codexAccountQuota(account.id));
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unable to load Codex quota";
      setCodexDetailsError(message);
      toast.error(message);
    } finally {
      setIsCodexDetailsLoading(false);
      await refresh();
    }
  }

  async function handleCreateModel(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await api.createModelCatalogEntry({
      id: createModelForm.id,
      displayName: createModelForm.displayName,
      family: createModelForm.family,
      enabled: createModelForm.enabled,
    });
    setCreateModelForm(emptyModelForm);
    setIsModelSheetOpen(false);
    toast.success("Model added");
    await refresh();
  }

  async function handleCreateRoute(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await api.createProviderModelRoute({
      publicModelId: createRouteForm.publicModelId,
      providerAccountId: createRouteForm.providerAccountId,
      upstreamModelId: createRouteForm.upstreamModelId,
      wireApi: createRouteForm.wireApi,
      role: createRouteForm.role,
      enabled: createRouteForm.enabled,
      stripParams: commaSeparatedValues(createRouteForm.stripParams),
    });
    setCreateRouteForm(emptyRouteForm);
    setIsRouteSheetOpen(false);
    toast.success("Provider route added");
    await refresh();
  }

  async function toggleApiKey(key: ApiKey) {
    await api.updateApiKey(key.id, { isActive: !key.isActive });
    toast.success(key.isActive ? "API key paused" : "API key activated");
    await refresh();
  }

  async function toggleModel(entry: ModelCatalogEntry) {
    await api.updateModelCatalogEntry(entry.id, { enabled: !entry.enabled });
    toast.success(entry.enabled ? "Model disabled" : "Model enabled");
    await refresh();
  }

  async function updateModelDetails(
    entry: ModelCatalogEntry,
    values: { displayName: string; family: string },
  ) {
    await api.updateModelCatalogEntry(entry.id, values);
    toast.success("Model updated");
    await refresh();
  }

  async function toggleRoute(route: ProviderModelRoute) {
    await api.updateProviderModelRoute(route.id, { enabled: !route.enabled });
    toast.success(route.enabled ? "Route disabled" : "Route enabled");
    await refresh();
  }

  async function deleteRoute(route: ProviderModelRoute) {
    if (!window.confirm(`Delete route for "${route.publicModelId}"?`)) {
      return false;
    }

    await api.deleteProviderModelRoute(route.id);
    toast.success("Provider route deleted");
    await refresh();
    return true;
  }

  async function deleteApiKey(key: ApiKey) {
    if (!window.confirm(`Delete API key "${key.name}"? This cannot be undone.`)) {
      return false;
    }

    await api.deleteApiKey(key.id);
    toast.success("API key deleted");
    await refresh();
    return true;
  }

  async function toggleAccount(account: ProviderAccount) {
    await api.updateProviderAccount(account.id, { isActive: !account.isActive });
    toast.success(account.isActive ? "Provider paused" : "Provider activated");
    await refresh();
  }

  async function deleteAccount(account: ProviderAccount) {
    const dependentRoutes = routeCountForAccount(modelRoutes, account.id);
    const routeWarning =
      dependentRoutes === 0
        ? ""
        : ` This will also delete ${dependentRoutes} provider route${dependentRoutes === 1 ? "" : "s"}.`;
    if (
      !window.confirm(
        `Delete provider account "${account.name}"? This cannot be undone.${routeWarning}`,
      )
    ) {
      return false;
    }

    await api.deleteProviderAccount(account.id);
    toast.success("Provider account deleted");
    await refresh();
    return true;
  }

  return {
    token,
    username,
    setUsername,
    password,
    setPassword,
    dashboard,
    apiKeys,
    accounts,
    providerPresets,
    modelCatalog,
    routableModels,
    modelRoutes,
    logs,
    isLoading,
    isKeySheetOpen,
    setIsKeySheetOpen,
    isAccountSheetOpen,
    editingAccount,
    isModelSheetOpen,
    setIsModelSheetOpen,
    isRouteSheetOpen,
    setIsRouteSheetOpen,
    createKeyForm,
    setCreateKeyForm,
    createAccountForm,
    setCreateAccountForm,
    createModelForm,
    setCreateModelForm,
    createRouteForm,
    setCreateRouteForm,
    createdSecret,
    setCreatedSecret,
    clientSetupApiKey,
    setClientSetupApiKey,
    geminiDetailsAccount,
    setGeminiDetailsAccount,
    geminiModels,
    geminiQuota,
    isGeminiDetailsLoading,
    geminiDetailsError,
    codexDetailsAccount,
    setCodexDetailsAccount,
    codexQuota,
    isCodexDetailsLoading,
    codexDetailsError,
    refresh,
    handleLogin,
    handleLogout,
    handleCreateKey,
    openCreateAccount,
    openEditAccount,
    handleAccountSheetOpenChange,
    handleSaveAccount,
    reconnectAntigravityAccount,
    inspectGeminiAccount,
    inspectCodexAccount,
    handleCreateModel,
    handleCreateRoute,
    toggleApiKey,
    toggleModel,
    updateModelDetails,
    toggleRoute,
    deleteRoute,
    deleteApiKey,
    toggleAccount,
    deleteAccount,
  };
}
