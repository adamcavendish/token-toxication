import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ActivityIcon,
  CableIcon,
  CheckIcon,
  ClipboardCopyIcon,
  DatabaseIcon,
  GaugeIcon,
  KeyRoundIcon,
  LayoutDashboardIcon,
  LogInIcon,
  LogOutIcon,
  PlusIcon,
  RefreshCcwIcon,
  RouteIcon,
  SettingsIcon,
  ShieldCheckIcon,
  TerminalSquareIcon,
  Trash2Icon,
} from "lucide-react";
import { toast } from "sonner";

import { api, clearStoredToken, getStoredToken, setStoredToken } from "./api";
import type {
  ApiKey,
  CodexAccountQuotaResponse,
  CodexAccountQuotaWindow,
  Dashboard,
  GeminiAccountModelsResponse,
  GeminiAccountQuotaResponse,
  ModelCatalogEntry,
  ProviderAccount,
  ProviderModelRoute,
  ProviderPreset,
  RequestLog,
} from "./types";
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { Toaster } from "@/components/ui/sonner";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

type View = "overview" | "keys" | "accounts" | "models" | "setup" | "logs" | "settings";

type CreateKeyForm = {
  name: string;
  description: string;
  permissions: string;
  rateLimitPerMinute: string;
  concurrencyLimit: string;
  dailyCostLimit: string;
};

type CreateAccountForm = {
  name: string;
  provider: string;
  baseUrl: string;
  authMode: string;
  wireApi: string;
  apiKey: string;
  isActive: boolean;
  priority: string;
};

type ModelCatalogForm = {
  id: string;
  displayName: string;
  family: string;
  enabled: boolean;
};

type ProviderRouteForm = {
  publicModelId: string;
  providerAccountId: string;
  upstreamModelId: string;
  wireApi: string;
  role: string;
  enabled: boolean;
  stripParams: string;
};

type ClientModelOption = {
  id: string;
  displayName: string;
};

type CodexQuotaRow = {
  limitId: string;
  displayName: string;
  windowName: string;
  window: CodexAccountQuotaWindow | null;
  allowed?: boolean | null;
  limitReached?: boolean | null;
};

type AntigravityOAuthMessage = {
  type: "token-toxication:antigravity-oauth";
  success: boolean;
  accountId?: string;
  error?: string;
};

const views: Array<{
  id: View;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}> = [
  { id: "overview", label: "Overview", icon: LayoutDashboardIcon },
  { id: "keys", label: "API Keys", icon: KeyRoundIcon },
  { id: "accounts", label: "Provider Accounts", icon: CableIcon },
  { id: "models", label: "Model Catalog", icon: DatabaseIcon },
  { id: "setup", label: "Client Setup", icon: TerminalSquareIcon },
  { id: "logs", label: "Request Log", icon: ActivityIcon },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

const emptyKeyForm: CreateKeyForm = {
  name: "",
  description: "",
  permissions: "all",
  rateLimitPerMinute: "0",
  concurrencyLimit: "0",
  dailyCostLimit: "0",
};

const emptyAccountForm: CreateAccountForm = {
  name: "",
  provider: "anthropic",
  baseUrl: "https://api.anthropic.com",
  authMode: "x-api-key",
  wireApi: "anthropic-messages",
  apiKey: "",
  isActive: true,
  priority: "0",
};

const emptyModelForm: ModelCatalogForm = {
  id: "",
  displayName: "",
  family: "other",
  enabled: true,
};

const emptyRouteForm: ProviderRouteForm = {
  publicModelId: "",
  providerAccountId: "",
  upstreamModelId: "",
  wireApi: "openai-chat",
  role: "primary",
  enabled: true,
  stripParams: "",
};

function App() {
  const [token, setToken] = useState(() => getStoredToken());
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [view, setView] = useState<View>("overview");
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [accounts, setAccounts] = useState<ProviderAccount[]>([]);
  const [providerPresets, setProviderPresets] = useState<ProviderPreset[]>([]);
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogEntry[]>([]);
  const [modelRoutes, setModelRoutes] = useState<ProviderModelRoute[]>([]);
  const [logs, setLogs] = useState<RequestLog[]>([]);
  const [isLoading, setIsLoading] = useState(Boolean(token));
  const [isKeySheetOpen, setIsKeySheetOpen] = useState(false);
  const [isAccountSheetOpen, setIsAccountSheetOpen] = useState(false);
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
        nextRoutes,
        nextLogs,
      ] = await Promise.all([
        api.dashboard(),
        api.apiKeys(),
        api.providerAccounts(),
        api.providerPresets(),
        api.modelCatalog(),
        api.providerModelRoutes(),
        api.requestLogs(50),
      ]);
      setDashboard(nextDashboard);
      setApiKeys(nextKeys);
      setAccounts(nextAccounts);
      setProviderPresets(nextPresets);
      setModelCatalog(nextCatalog);
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

  async function handleCreateAccount(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (isAntigravityAccountAuth(createAccountForm.authMode)) {
      await launchAntigravityOAuth({
        name: createAccountForm.name,
        priority: numberFromInput(createAccountForm.priority),
      });
      return;
    }
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
    setCreateAccountForm(emptyAccountForm);
    setIsAccountSheetOpen(false);
    toast.success("Provider account created");
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
      return;
    }

    await api.deleteProviderModelRoute(route.id);
    toast.success("Provider route deleted");
    await refresh();
  }

  async function deleteApiKey(key: ApiKey) {
    if (!window.confirm(`Delete API key "${key.name}"? This cannot be undone.`)) {
      return;
    }

    await api.deleteApiKey(key.id);
    toast.success("API key deleted");
    await refresh();
  }

  async function toggleAccount(account: ProviderAccount) {
    await api.updateProviderAccount(account.id, { isActive: !account.isActive });
    toast.success(account.isActive ? "Provider paused" : "Provider activated");
    await refresh();
  }

  if (!token) {
    return (
      <TooltipProvider>
        <main className="min-h-svh bg-background text-foreground">
          <div className="mx-auto flex min-h-svh w-full max-w-md flex-col justify-center gap-6 px-6">
            <div className="flex flex-col gap-2">
              <div className="flex size-11 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                <RouteIcon className="size-5" />
              </div>
              <h1 className="text-2xl font-semibold">Token Toxication</h1>
              <p className="text-sm text-muted-foreground">Relay operations console</p>
            </div>
            <Card>
              <CardHeader>
                <CardTitle>Admin sign in</CardTitle>
                <CardDescription>
                  Use the credentials from your service environment.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <form className="flex flex-col gap-4" onSubmit={handleLogin}>
                  <div className="flex flex-col gap-2">
                    <Label htmlFor="username">Username</Label>
                    <Input
                      id="username"
                      value={username}
                      onChange={(event) => setUsername(event.target.value)}
                      autoComplete="username"
                    />
                  </div>
                  <div className="flex flex-col gap-2">
                    <Label htmlFor="password">Password</Label>
                    <Input
                      id="password"
                      type="password"
                      value={password}
                      onChange={(event) => setPassword(event.target.value)}
                      autoComplete="current-password"
                    />
                  </div>
                  <Button type="submit">
                    <ShieldCheckIcon data-icon="inline-start" />
                    Sign in
                  </Button>
                </form>
              </CardContent>
            </Card>
          </div>
        </main>
        <Toaster />
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider>
      <div className="min-h-svh bg-background text-foreground">
        <div className="grid min-h-svh grid-cols-1 lg:grid-cols-[260px_1fr]">
          <aside className="border-b bg-sidebar text-sidebar-foreground lg:border-r lg:border-b-0">
            <div className="flex h-full flex-col gap-6 p-4">
              <div className="flex items-center gap-3 px-2">
                <div className="flex size-9 items-center justify-center rounded-md bg-sidebar-primary text-sidebar-primary-foreground">
                  <RouteIcon className="size-4" />
                </div>
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">Token Toxication</div>
                  <div className="text-xs text-muted-foreground">Relay control plane</div>
                </div>
              </div>
              <nav className="grid gap-1">
                {views.map((item) => {
                  const Icon = item.icon;
                  return (
                    <Button
                      key={item.id}
                      type="button"
                      variant={view === item.id ? "secondary" : "ghost"}
                      className="justify-start"
                      onClick={() => setView(item.id)}
                    >
                      <Icon data-icon="inline-start" />
                      {item.label}
                    </Button>
                  );
                })}
              </nav>
            </div>
          </aside>

          <main className="min-w-0">
            <header className="sticky top-0 z-10 border-b bg-background/95 backdrop-blur">
              <div className="flex min-h-16 items-center justify-between gap-3 px-5">
                <div className="min-w-0">
                  <div className="text-sm text-muted-foreground">Environment</div>
                  <div className="flex items-center gap-2">
                    <Badge variant="outline">
                      <CheckIcon className="size-3" />
                      Local
                    </Badge>
                    <span className="truncate text-sm font-medium">{currentViewLabel(view)}</span>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Button type="button" variant="outline" onClick={refresh}>
                    <RefreshCcwIcon data-icon="inline-start" />
                    Refresh
                  </Button>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button type="button" variant="secondary">
                        <TerminalSquareIcon data-icon="inline-start" />
                        Admin
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onClick={handleLogout}>
                        <LogOutIcon className="size-4" />
                        Sign out
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>
            </header>

            <div className="flex flex-col gap-5 p-5">
              {isLoading ? (
                <LoadingState />
              ) : (
                <>
                  {view === "overview" && dashboard ? (
                    <Overview
                      dashboard={dashboard}
                      onCreateKey={() => setIsKeySheetOpen(true)}
                      onCreateAccount={() => setIsAccountSheetOpen(true)}
                    />
                  ) : null}
                  {view === "keys" ? (
                    <ApiKeysView
                      apiKeys={apiKeys}
                      onCreate={() => setIsKeySheetOpen(true)}
                      onToggle={toggleApiKey}
                      onDelete={deleteApiKey}
                    />
                  ) : null}
                  {view === "accounts" ? (
                    <AccountsView
                      accounts={accounts}
                      routes={modelRoutes}
                      onCreate={() => setIsAccountSheetOpen(true)}
                      onToggle={toggleAccount}
                      onInspectCodex={inspectCodexAccount}
                      onInspectGemini={inspectGeminiAccount}
                      onReconnectAntigravity={reconnectAntigravityAccount}
                    />
                  ) : null}
                  {view === "models" ? (
                    <ModelCatalogView
                      accounts={accounts}
                      models={modelCatalog}
                      routes={modelRoutes}
                      onCreateModel={() => setIsModelSheetOpen(true)}
                      onCreateRoute={() => setIsRouteSheetOpen(true)}
                      onUpdateModel={updateModelDetails}
                      onToggleModel={toggleModel}
                      onToggleRoute={toggleRoute}
                      onDeleteRoute={deleteRoute}
                    />
                  ) : null}
                  {view === "setup" ? (
                    <ClientSetupView
                      accounts={accounts}
                      models={modelCatalog}
                      routes={modelRoutes}
                      apiKey={clientSetupApiKey}
                      setApiKey={setClientSetupApiKey}
                    />
                  ) : null}
                  {view === "logs" ? <RequestLogsView logs={logs} /> : null}
                  {view === "settings" ? <SettingsView /> : null}
                </>
              )}
            </div>
          </main>
        </div>
      </div>

      <CreateKeySheet
        open={isKeySheetOpen}
        form={createKeyForm}
        setForm={setCreateKeyForm}
        onOpenChange={setIsKeySheetOpen}
        onSubmit={handleCreateKey}
      />
      <CreateAccountSheet
        open={isAccountSheetOpen}
        form={createAccountForm}
        setForm={setCreateAccountForm}
        presets={providerPresets}
        onOpenChange={setIsAccountSheetOpen}
        onSubmit={handleCreateAccount}
      />
      <CreateModelSheet
        open={isModelSheetOpen}
        form={createModelForm}
        setForm={setCreateModelForm}
        onOpenChange={setIsModelSheetOpen}
        onSubmit={handleCreateModel}
      />
      <CreateRouteSheet
        open={isRouteSheetOpen}
        form={createRouteForm}
        setForm={setCreateRouteForm}
        models={modelCatalog}
        accounts={accounts}
        onOpenChange={setIsRouteSheetOpen}
        onSubmit={handleCreateRoute}
      />
      <Dialog
        open={Boolean(createdSecret)}
        onOpenChange={(open) => !open && setCreatedSecret(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>API key secret</DialogTitle>
            <DialogDescription>This secret is shown once.</DialogDescription>
          </DialogHeader>
          <Alert>
            <KeyRoundIcon className="size-4" />
            <AlertTitle>Store this value now</AlertTitle>
            <AlertDescription className="break-all font-mono text-xs">
              {createdSecret}
            </AlertDescription>
          </Alert>
          <div className="grid gap-2 sm:grid-cols-2">
            <Button type="button" onClick={() => createdSecret && copyText(createdSecret)}>
              <ClipboardCopyIcon data-icon="inline-start" />
              Copy secret
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => {
                setView("setup");
                setCreatedSecret(null);
              }}
            >
              <TerminalSquareIcon data-icon="inline-start" />
              Client setup
            </Button>
          </div>
        </DialogContent>
      </Dialog>
      <GeminiAccountDialog
        account={geminiDetailsAccount}
        models={geminiModels}
        quota={geminiQuota}
        loading={isGeminiDetailsLoading}
        error={geminiDetailsError}
        onOpenChange={(open) => !open && setGeminiDetailsAccount(null)}
      />
      <CodexAccountDialog
        account={codexDetailsAccount}
        quota={codexQuota}
        loading={isCodexDetailsLoading}
        error={codexDetailsError}
        onOpenChange={(open) => !open && setCodexDetailsAccount(null)}
      />
      <Toaster />
    </TooltipProvider>
  );
}

function Overview({
  dashboard,
  onCreateKey,
  onCreateAccount,
}: {
  dashboard: Dashboard;
  onCreateKey: () => void;
  onCreateAccount: () => void;
}) {
  const trend = useMemo(() => buildTrend(dashboard.recentRequests), [dashboard.recentRequests]);
  return (
    <div className="flex flex-col gap-5">
      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          title="Requests today"
          value={formatNumber(dashboard.usage.requestsToday)}
          detail={`${formatNumber(dashboard.usage.totalRequests)} total`}
          icon={ActivityIcon}
        />
        <MetricCard
          title="Tokens today"
          value={formatNumber(dashboard.usage.tokensToday)}
          detail={`${formatNumber(dashboard.usage.totalTokens)} total`}
          icon={TerminalSquareIcon}
        />
        <MetricCard
          title="Active API keys"
          value={`${dashboard.activeApiKeys}/${dashboard.totalApiKeys}`}
          detail="usable client credentials"
          icon={KeyRoundIcon}
        />
        <MetricCard
          title="Healthy accounts"
          value={`${dashboard.healthyAccounts}/${dashboard.totalAccounts}`}
          detail="active upstream accounts"
          icon={CableIcon}
        />
      </section>

      <section className="grid gap-5 xl:grid-cols-[1.25fr_0.75fr]">
        <Card>
          <CardHeader className="flex-row items-start justify-between gap-4">
            <div className="flex flex-col gap-1">
              <CardTitle>Request flow</CardTitle>
              <CardDescription>Recent relay volume across the local log window.</CardDescription>
            </div>
            <Button type="button" onClick={onCreateKey}>
              <PlusIcon data-icon="inline-start" />
              API Key
            </Button>
          </CardHeader>
          <CardContent>
            <TrendChart values={trend} />
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex-row items-start justify-between gap-4">
            <div className="flex flex-col gap-1">
              <CardTitle>Provider pool</CardTitle>
              <CardDescription>Routing health and availability.</CardDescription>
            </div>
            <Button type="button" variant="outline" onClick={onCreateAccount}>
              <PlusIcon data-icon="inline-start" />
              Account
            </Button>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            {dashboard.accounts.length === 0 ? (
              <EmptyNotice
                title="No provider accounts"
                body="Add a provider account to relay traffic."
              />
            ) : (
              dashboard.accounts.slice(0, 5).map((account) => (
                <div key={account.id} className="flex items-center justify-between gap-4">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">{account.name}</div>
                    <div className="truncate text-xs text-muted-foreground">
                      {account.provider} · {wireApiLabel(account.wireApi)}
                    </div>
                  </div>
                  {statusBadge(account.status, account.isActive)}
                </div>
              ))
            )}
          </CardContent>
        </Card>
      </section>

      <RequestLogsView logs={dashboard.recentRequests} compact />
    </div>
  );
}

function MetricCard({
  title,
  value,
  detail,
  icon: Icon,
}: {
  title: string;
  value: string;
  detail: string;
  icon: React.ComponentType<{ className?: string }>;
}) {
  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between gap-3 pb-2">
        <CardTitle className="text-sm font-medium">{title}</CardTitle>
        <Icon className="size-4 text-muted-foreground" />
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <div className="text-2xl font-semibold">{value}</div>
        <div className="text-xs text-muted-foreground">{detail}</div>
      </CardContent>
    </Card>
  );
}

function ApiKeysView({
  apiKeys,
  onCreate,
  onToggle,
  onDelete,
}: {
  apiKeys: ApiKey[];
  onCreate: () => void;
  onToggle: (key: ApiKey) => void;
  onDelete: (key: ApiKey) => void;
}) {
  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between gap-4">
        <div className="flex flex-col gap-1">
          <CardTitle>API Keys</CardTitle>
          <CardDescription>Client credentials, limits, and service permissions.</CardDescription>
        </div>
        <Button type="button" onClick={onCreate}>
          <PlusIcon data-icon="inline-start" />
          Create
        </Button>
      </CardHeader>
      <CardContent>
        <div className="hidden md:block">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Preview</TableHead>
                <TableHead>Permissions</TableHead>
                <TableHead>Limits</TableHead>
                <TableHead>Last used</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {apiKeys.map((key) => (
                <TableRow key={key.id}>
                  <TableCell>
                    <div className="flex flex-col gap-1">
                      <span className="font-medium">{key.name}</span>
                      <span className="text-xs text-muted-foreground">
                        {key.description || "No description"}
                      </span>
                    </div>
                  </TableCell>
                  <TableCell className="font-mono text-xs">{key.keyPreview}</TableCell>
                  <TableCell>
                    {key.permissions.length === 0 ? "All" : key.permissions.join(", ")}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-col gap-1 text-xs text-muted-foreground">
                      <span>{key.rateLimitPerMinute || "No"} rpm</span>
                      <span>{key.concurrencyLimit || "No"} concurrent</span>
                    </div>
                  </TableCell>
                  <TableCell>{formatDate(key.lastUsedAt)}</TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-2">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => onToggle(key)}
                      >
                        {key.isActive ? "Active" : "Paused"}
                      </Button>
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <Button
                            type="button"
                            variant="destructive"
                            size="icon-sm"
                            onClick={() => onDelete(key)}
                          >
                            <Trash2Icon />
                          </Button>
                        </TooltipTrigger>
                        <TooltipContent>Delete API key</TooltipContent>
                      </Tooltip>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
              {apiKeys.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6}>
                    <EmptyNotice
                      title="No API keys"
                      body="Create a key before connecting a client."
                    />
                  </TableCell>
                </TableRow>
              ) : null}
            </TableBody>
          </Table>
        </div>
        <div className="grid gap-3 md:hidden">
          {apiKeys.length === 0 ? (
            <EmptyNotice title="No API keys" body="Create a key before connecting a client." />
          ) : (
            apiKeys.map((key) => (
              <div key={key.id} className="flex flex-col gap-3 rounded-md border p-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">{key.name}</div>
                    <div className="truncate font-mono text-xs text-muted-foreground">
                      {key.keyPreview}
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-2">
                    <Button type="button" variant="outline" size="sm" onClick={() => onToggle(key)}>
                      {key.isActive ? "Active" : "Paused"}
                    </Button>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          type="button"
                          variant="destructive"
                          size="icon-sm"
                          onClick={() => onDelete(key)}
                        >
                          <Trash2Icon />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Delete API key</TooltipContent>
                    </Tooltip>
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground">
                  <span>
                    {key.permissions.length === 0 ? "All services" : key.permissions.join(", ")}
                  </span>
                  <span>{formatDate(key.lastUsedAt)}</span>
                  <span>{key.rateLimitPerMinute || "No"} rpm</span>
                  <span>{key.concurrencyLimit || "No"} concurrent</span>
                </div>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function AccountsView({
  accounts,
  routes,
  onCreate,
  onToggle,
  onInspectCodex,
  onInspectGemini,
  onReconnectAntigravity,
}: {
  accounts: ProviderAccount[];
  routes: ProviderModelRoute[];
  onCreate: () => void;
  onToggle: (account: ProviderAccount) => void;
  onInspectCodex: (account: ProviderAccount) => void;
  onInspectGemini: (account: ProviderAccount) => void;
  onReconnectAntigravity: (account: ProviderAccount) => void;
}) {
  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between gap-4">
        <div className="flex flex-col gap-1">
          <CardTitle>Provider Accounts</CardTitle>
          <CardDescription>Upstream credentials used by model routes.</CardDescription>
        </div>
        <Button type="button" onClick={onCreate}>
          <PlusIcon data-icon="inline-start" />
          Add Account
        </Button>
      </CardHeader>
      <CardContent>
        <div className="hidden md:block">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Provider</TableHead>
                <TableHead>Protocol</TableHead>
                <TableHead>Base URL</TableHead>
                <TableHead>Routes</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {accounts.map((account) => (
                <TableRow key={account.id}>
                  <TableCell className="font-medium">{account.name}</TableCell>
                  <TableCell>{account.provider}</TableCell>
                  <TableCell>{wireApiLabel(account.wireApi)}</TableCell>
                  <TableCell className="max-w-[280px] truncate">{account.baseUrl}</TableCell>
                  <TableCell>{routeCountForAccount(routes, account.id)}</TableCell>
                  <TableCell>{statusBadge(account.status, account.isActive)}</TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-2">
                      {isCodexAccount(account) ? (
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="outline"
                              size="icon-sm"
                              aria-label="Codex quota"
                              onClick={() => onInspectCodex(account)}
                            >
                              <GaugeIcon />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>Codex quota</TooltipContent>
                        </Tooltip>
                      ) : null}
                      {isGeminiAccount(account) ? (
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="outline"
                              size="icon-sm"
                              aria-label="Models and quota"
                              onClick={() => onInspectGemini(account)}
                            >
                              <GaugeIcon />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>Models and quota</TooltipContent>
                        </Tooltip>
                      ) : null}
                      {isGeminiAccount(account) ? (
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="outline"
                              size="icon-sm"
                              aria-label="Reconnect Antigravity"
                              onClick={() => onReconnectAntigravity(account)}
                            >
                              <LogInIcon />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>Reconnect Antigravity</TooltipContent>
                        </Tooltip>
                      ) : null}
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => onToggle(account)}
                      >
                        {account.isActive ? "Enabled" : "Disabled"}
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
              {accounts.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={7}>
                    <EmptyNotice
                      title="No provider accounts"
                      body="Add an account to make the relay schedulable."
                    />
                  </TableCell>
                </TableRow>
              ) : null}
            </TableBody>
          </Table>
        </div>
        <div className="grid gap-3 md:hidden">
          {accounts.length === 0 ? (
            <EmptyNotice
              title="No provider accounts"
              body="Add an account to make the relay schedulable."
            />
          ) : (
            accounts.map((account) => (
              <div key={account.id} className="flex flex-col gap-3 rounded-md border p-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">{account.name}</div>
                    <div className="truncate text-xs text-muted-foreground">{account.baseUrl}</div>
                  </div>
                  {statusBadge(account.status, account.isActive)}
                </div>
                <div className="flex flex-wrap gap-2">
                  <Badge variant="outline">{account.provider}</Badge>
                  <Badge variant="secondary">{wireApiLabel(account.wireApi)}</Badge>
                </div>
                <div className="flex items-center justify-between gap-3">
                  <span className="text-xs text-muted-foreground">
                    {routeCountForAccount(routes, account.id)} routes
                  </span>
                  <div className="flex items-center gap-2">
                    {isCodexAccount(account) ? (
                      <Button
                        type="button"
                        variant="outline"
                        size="icon-sm"
                        aria-label="Codex quota"
                        onClick={() => onInspectCodex(account)}
                      >
                        <GaugeIcon />
                      </Button>
                    ) : null}
                    {isGeminiAccount(account) ? (
                      <Button
                        type="button"
                        variant="outline"
                        size="icon-sm"
                        aria-label="Models and quota"
                        onClick={() => onInspectGemini(account)}
                      >
                        <GaugeIcon />
                      </Button>
                    ) : null}
                    {isGeminiAccount(account) ? (
                      <Button
                        type="button"
                        variant="outline"
                        size="icon-sm"
                        aria-label="Reconnect Antigravity"
                        onClick={() => onReconnectAntigravity(account)}
                      >
                        <LogInIcon />
                      </Button>
                    ) : null}
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => onToggle(account)}
                    >
                      {account.isActive ? "Enabled" : "Disabled"}
                    </Button>
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function GeminiAccountDialog({
  account,
  models,
  quota,
  loading,
  error,
  onOpenChange,
}: {
  account: ProviderAccount | null;
  models: GeminiAccountModelsResponse | null;
  quota: GeminiAccountQuotaResponse | null;
  loading: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
}) {
  const rows = useMemo(() => {
    const entries = new Map<string, { id: string; displayName: string }>();
    models?.models.forEach((model) => entries.set(model.id, model));
    quota?.quotas.forEach((item) => {
      if (!entries.has(item.modelId)) {
        entries.set(item.modelId, { id: item.modelId, displayName: item.modelId });
      }
    });
    return [...entries.values()].sort((left, right) => left.id.localeCompare(right.id));
  }, [models, quota]);
  const quotaByModel = useMemo(
    () => new Map(quota?.quotas.map((item) => [item.modelId, item]) ?? []),
    [quota],
  );
  const quotaSummaryRows = useMemo(() => {
    const summary = quota?.quotaSummary;
    if (!summary) {
      return [];
    }
    const standalone = summary.buckets.map((bucket) => ({
      group: summary.description || "Account",
      groupDescription: null,
      bucket,
    }));
    const grouped = summary.groups.flatMap((group) =>
      group.buckets.map((bucket) => ({
        group: group.displayName,
        groupDescription: group.description,
        bucket,
      })),
    );
    return [...standalone, ...grouped];
  }, [quota]);

  return (
    <Dialog open={Boolean(account)} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[86svh] overflow-y-auto sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>{account?.name || "Gemini account"}</DialogTitle>
          <DialogDescription>Models and quota reported by this Google account.</DialogDescription>
        </DialogHeader>
        {loading ? (
          <div className="grid gap-3">
            <Skeleton className="h-16" />
            <Skeleton className="h-52" />
          </div>
        ) : error ? (
          <Alert variant="destructive">
            <ActivityIcon className="size-4" />
            <AlertTitle>Unable to load account data</AlertTitle>
            <AlertDescription className="break-words">{error}</AlertDescription>
          </Alert>
        ) : (
          <div className="flex min-w-0 flex-col gap-4">
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
              <SettingRow label="Project" value={quota?.project || models?.project || "unknown"} />
              <SettingRow label="Auth" value={quota?.authMode || account?.authMode || "unknown"} />
              <SettingRow label="Tier" value={formatGeminiTier(quota?.currentTier)} />
              <SettingRow label="Quota source" value={quota?.quotaSource || "unknown"} />
            </div>
            {quota?.paidTier ? (
              <Alert>
                <ShieldCheckIcon className="size-4" />
                <AlertTitle>{quota.paidTier.name || quota.paidTier.id}</AlertTitle>
                <AlertDescription>{quota.paidTier.description}</AlertDescription>
              </Alert>
            ) : null}
            {quota?.quotaSummaryError ? (
              <Alert>
                <ActivityIcon className="size-4" />
                <AlertTitle>Quota summary unavailable</AlertTitle>
                <AlertDescription className="break-words">
                  {quota.quotaSummaryError}
                </AlertDescription>
              </Alert>
            ) : null}
            {quotaSummaryRows.length > 0 ? (
              <div className="flex flex-col gap-2">
                <div className="text-sm font-medium">Usage windows</div>
                <div className="w-full min-w-0 max-w-full overflow-x-auto rounded-md border">
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Model group</TableHead>
                        <TableHead>Window</TableHead>
                        <TableHead className="min-w-48">Remaining</TableHead>
                        <TableHead>Reset</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {quotaSummaryRows.map(({ group, groupDescription, bucket }) => {
                        const percent = quotaPercent(bucket.remainingFraction);
                        return (
                          <TableRow key={`${group}:${bucket.bucketId}`}>
                            <TableCell>
                              <div className="font-medium">{group}</div>
                              {groupDescription ? (
                                <div className="max-w-72 text-xs text-muted-foreground">
                                  {groupDescription}
                                </div>
                              ) : null}
                            </TableCell>
                            <TableCell>
                              <div>{bucket.displayName || bucket.bucketId}</div>
                              {bucket.description ? (
                                <div className="max-w-80 text-xs text-muted-foreground">
                                  {bucket.description}
                                </div>
                              ) : null}
                            </TableCell>
                            <TableCell>
                              {percent === undefined ? (
                                <span className="text-xs text-muted-foreground">unknown</span>
                              ) : (
                                <div className="flex min-w-40 items-center gap-3">
                                  <Progress value={percent} className="min-w-24" />
                                  <span className="w-16 text-right font-mono text-xs">
                                    {formatQuotaPercent(percent)}
                                  </span>
                                </div>
                              )}
                            </TableCell>
                            <TableCell>{formatDate(bucket.resetTime)}</TableCell>
                          </TableRow>
                        );
                      })}
                    </TableBody>
                  </Table>
                </div>
              </div>
            ) : null}
            <div className="w-full min-w-0 max-w-full overflow-x-auto rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Model</TableHead>
                    <TableHead>Model ID</TableHead>
                    <TableHead className="min-w-48">Remaining</TableHead>
                    <TableHead>Reset</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {rows.map((model) => {
                    const modelQuota = quotaByModel.get(model.id);
                    const remaining = modelQuota?.remainingFraction;
                    const percent = quotaPercent(remaining);
                    return (
                      <TableRow key={model.id}>
                        <TableCell className="font-medium">{model.displayName}</TableCell>
                        <TableCell className="font-mono text-xs">{model.id}</TableCell>
                        <TableCell>
                          {percent === undefined ? (
                            <span className="text-xs text-muted-foreground">unknown</span>
                          ) : (
                            <div className="flex min-w-40 items-center gap-3">
                              <Progress value={percent} className="min-w-24" />
                              <span className="w-14 text-right font-mono text-xs">
                                {percent.toFixed(1)}%
                              </span>
                            </div>
                          )}
                        </TableCell>
                        <TableCell>{formatDate(modelQuota?.resetTime)}</TableCell>
                      </TableRow>
                    );
                  })}
                  {rows.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={4}>
                        <EmptyNotice
                          title="No account models returned"
                          body="Google did not return models for this credential."
                        />
                      </TableCell>
                    </TableRow>
                  ) : null}
                </TableBody>
              </Table>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function CodexAccountDialog({
  account,
  quota,
  loading,
  error,
  onOpenChange,
}: {
  account: ProviderAccount | null;
  quota: CodexAccountQuotaResponse | null;
  loading: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
}) {
  const rows = useMemo<CodexQuotaRow[]>(() => {
    const next: CodexQuotaRow[] = [];
    quota?.limits.forEach((limit) => {
      const windows = [
        { name: "Primary", window: limit.primaryWindow },
        { name: "Secondary", window: limit.secondaryWindow },
      ].filter((entry): entry is { name: string; window: CodexAccountQuotaWindow } =>
        Boolean(entry.window),
      );
      if (windows.length === 0) {
        next.push({
          limitId: limit.limitId,
          displayName: limit.displayName,
          windowName: "Unreported",
          window: null,
          allowed: limit.allowed,
          limitReached: limit.limitReached,
        });
        return;
      }
      windows.forEach(({ name, window }) => {
        next.push({
          limitId: limit.limitId,
          displayName: limit.displayName,
          windowName: name,
          window,
          allowed: limit.allowed,
          limitReached: limit.limitReached,
        });
      });
    });
    return next;
  }, [quota]);
  const spendLimit = quota?.spendControl?.individualLimit;

  return (
    <Dialog open={Boolean(account)} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[86svh] overflow-y-auto sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>{account?.name || "Codex account"}</DialogTitle>
          <DialogDescription>Subscription quota reported by this Codex account.</DialogDescription>
        </DialogHeader>
        {loading ? (
          <div className="grid gap-3">
            <Skeleton className="h-16" />
            <Skeleton className="h-52" />
          </div>
        ) : error ? (
          <Alert variant="destructive">
            <ActivityIcon className="size-4" />
            <AlertTitle>Unable to load quota</AlertTitle>
            <AlertDescription className="break-words">{error}</AlertDescription>
          </Alert>
        ) : (
          <div className="flex min-w-0 flex-col gap-4">
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
              <SettingRow label="Plan" value={quota?.planType || "unknown"} />
              <SettingRow label="Auth" value={quota?.authMode || account?.authMode || "unknown"} />
              <SettingRow label="Limits" value={String(quota?.limits.length ?? 0)} />
              <SettingRow
                label="Reset credits"
                value={formatOptionalNumber(quota?.resetCreditsAvailableCount)}
              />
            </div>
            {quota?.endpoint ? <SettingRow label="Relay endpoint" value={quota.endpoint} /> : null}
            {quota?.rateLimitReachedType ? (
              <Alert variant="destructive">
                <ActivityIcon className="size-4" />
                <AlertTitle>Quota unavailable</AlertTitle>
                <AlertDescription>
                  {humanizeIdentifier(quota.rateLimitReachedType)}
                </AlertDescription>
              </Alert>
            ) : null}
            {quota?.credits || quota?.spendControl ? (
              <div className="grid gap-3 sm:grid-cols-2">
                {quota.credits ? (
                  <SettingRow label="Credits" value={formatCodexCredits(quota.credits)} />
                ) : null}
                {quota.spendControl ? (
                  <SettingRow
                    label="Spending limit"
                    value={formatCodexSpendControl(quota.spendControl.reached, spendLimit)}
                  />
                ) : null}
              </div>
            ) : null}
            <div className="w-full min-w-0 max-w-full overflow-x-auto rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Limit</TableHead>
                    <TableHead>Window</TableHead>
                    <TableHead className="min-w-48">Used</TableHead>
                    <TableHead>Reset</TableHead>
                    <TableHead>Status</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {rows.map((row) => {
                    const usedPercent = codexUsedPercent(row.window?.usedPercent);
                    return (
                      <TableRow key={`${row.limitId}:${row.windowName}`}>
                        <TableCell>
                          <div className="font-medium">{row.displayName}</div>
                          <div className="font-mono text-xs text-muted-foreground">
                            {row.limitId}
                          </div>
                        </TableCell>
                        <TableCell>
                          <div>{formatCodexWindow(row.window?.limitWindowSeconds)}</div>
                          <div className="text-xs text-muted-foreground">{row.windowName}</div>
                        </TableCell>
                        <TableCell>
                          {usedPercent === undefined ? (
                            <span className="text-xs text-muted-foreground">unknown</span>
                          ) : (
                            <div className="flex min-w-40 items-center gap-3">
                              <Progress value={usedPercent} className="min-w-24" />
                              <span className="w-16 text-right font-mono text-xs">
                                {formatQuotaPercent(usedPercent)}
                              </span>
                            </div>
                          )}
                        </TableCell>
                        <TableCell>{formatQuotaReset(row.window?.resetAt)}</TableCell>
                        <TableCell>{codexQuotaStatus(row.allowed, row.limitReached)}</TableCell>
                      </TableRow>
                    );
                  })}
                  {rows.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={5}>
                        <EmptyNotice
                          title="No quota windows returned"
                          body="The relay returned no Codex quota windows."
                        />
                      </TableCell>
                    </TableRow>
                  ) : null}
                </TableBody>
              </Table>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function ModelCatalogView({
  accounts,
  models,
  routes,
  onCreateModel,
  onCreateRoute,
  onUpdateModel,
  onToggleModel,
  onToggleRoute,
  onDeleteRoute,
}: {
  accounts: ProviderAccount[];
  models: ModelCatalogEntry[];
  routes: ProviderModelRoute[];
  onCreateModel: () => void;
  onCreateRoute: () => void;
  onUpdateModel: (
    entry: ModelCatalogEntry,
    values: { displayName: string; family: string },
  ) => void;
  onToggleModel: (entry: ModelCatalogEntry) => void;
  onToggleRoute: (route: ProviderModelRoute) => void;
  onDeleteRoute: (route: ProviderModelRoute) => void;
}) {
  const [query, setQuery] = useState("");
  const [family, setFamily] = useState("all");
  const [drafts, setDrafts] = useState<Record<string, { displayName: string; family: string }>>({});
  const families = useMemo(
    () => ["all", ...Array.from(new Set(models.map((model) => model.family))).sort()],
    [models],
  );
  const filteredModels = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return models.filter((model) => {
      const matchesFamily = family === "all" || model.family === family;
      const matchesQuery =
        needle === "" ||
        model.id.toLowerCase().includes(needle) ||
        model.displayName.toLowerCase().includes(needle) ||
        model.family.toLowerCase().includes(needle);
      return matchesFamily && matchesQuery;
    });
  }, [family, models, query]);

  useEffect(() => {
    setDrafts((current) => {
      const next: Record<string, { displayName: string; family: string }> = {};
      models.forEach((model) => {
        next[model.id] = current[model.id] ?? {
          displayName: model.displayName,
          family: model.family,
        };
      });
      return next;
    });
  }, [models]);

  return (
    <div className="flex flex-col gap-5">
      <Card>
        <CardHeader className="flex-row items-start justify-between gap-4">
          <div className="flex flex-col gap-1">
            <CardTitle>Model Catalog</CardTitle>
            <CardDescription>Every exact model name clients may send.</CardDescription>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button type="button" variant="outline" onClick={onCreateRoute}>
              <RouteIcon data-icon="inline-start" />
              Add Route
            </Button>
            <Button type="button" onClick={onCreateModel}>
              <PlusIcon data-icon="inline-start" />
              Add Model
            </Button>
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]">
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search model, display name, or family"
            />
            <div className="flex flex-wrap gap-2">
              {families.map((item) => (
                <Button
                  key={item}
                  type="button"
                  size="xs"
                  variant={family === item ? "secondary" : "outline"}
                  onClick={() => setFamily(item)}
                >
                  {item}
                </Button>
              ))}
            </div>
          </div>
          <div className="hidden md:block">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Model</TableHead>
                  <TableHead>Display Name</TableHead>
                  <TableHead>Family</TableHead>
                  <TableHead>Routes</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredModels.map((model) => {
                  const draft = drafts[model.id] ?? {
                    displayName: model.displayName,
                    family: model.family,
                  };
                  const changed =
                    draft.displayName !== model.displayName || draft.family !== model.family;
                  return (
                    <TableRow key={model.id}>
                      <TableCell className="font-mono text-xs">{model.id}</TableCell>
                      <TableCell>
                        <Input
                          value={draft.displayName}
                          onChange={(event) =>
                            setDrafts((current) => ({
                              ...current,
                              [model.id]: { ...draft, displayName: event.target.value },
                            }))
                          }
                        />
                      </TableCell>
                      <TableCell>
                        <Input
                          value={draft.family}
                          onChange={(event) =>
                            setDrafts((current) => ({
                              ...current,
                              [model.id]: { ...draft, family: event.target.value },
                            }))
                          }
                        />
                      </TableCell>
                      <TableCell>{routeCountForModel(routes, model.id)}</TableCell>
                      <TableCell>
                        {model.enabled
                          ? statusBadge("healthy", true)
                          : statusBadge("paused", false)}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-2">
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={!changed}
                            onClick={() => onUpdateModel(model, draft)}
                          >
                            Save
                          </Button>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => onToggleModel(model)}
                          >
                            {model.enabled ? "Disable" : "Enable"}
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })}
                {filteredModels.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6}>
                      <EmptyNotice
                        title="No catalog models"
                        body="Add exact public model names before creating provider routes."
                      />
                    </TableCell>
                  </TableRow>
                ) : null}
              </TableBody>
            </Table>
          </div>
          <div className="grid gap-3 md:hidden">
            {filteredModels.length === 0 ? (
              <EmptyNotice
                title="No catalog models"
                body="Add exact public model names before creating provider routes."
              />
            ) : (
              filteredModels.map((model) => (
                <div key={model.id} className="flex flex-col gap-3 rounded-md border p-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="truncate font-mono text-sm">{model.id}</div>
                      <div className="truncate text-xs text-muted-foreground">
                        {model.displayName}
                      </div>
                    </div>
                    {model.enabled ? statusBadge("healthy", true) : statusBadge("paused", false)}
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Badge variant="outline">{model.family}</Badge>
                    <Badge variant="secondary">{routeCountForModel(routes, model.id)} routes</Badge>
                  </div>
                </div>
              ))
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Provider Model Routes</CardTitle>
          <CardDescription>
            Primary and backup bindings from public models to upstream models.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Public Model</TableHead>
                <TableHead>Upstream Model</TableHead>
                <TableHead>Provider Account</TableHead>
                <TableHead>Protocol</TableHead>
                <TableHead>Role</TableHead>
                <TableHead>Policy</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {routes.map((route) => (
                <TableRow key={route.id}>
                  <TableCell className="font-mono text-xs">{route.publicModelId}</TableCell>
                  <TableCell className="font-mono text-xs">{route.upstreamModelId}</TableCell>
                  <TableCell>{accountName(accounts, route.providerAccountId)}</TableCell>
                  <TableCell>{wireApiLabel(route.wireApi)}</TableCell>
                  <TableCell>{routeRoleBadge(route.role)}</TableCell>
                  <TableCell className="max-w-[220px] text-xs text-muted-foreground">
                    {formatRoutePolicy(route)}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-col gap-1">
                      {statusBadge(route.status, route.enabled)}
                      {route.cooldownUntil ? (
                        <span className="text-xs text-muted-foreground">
                          until {formatDate(route.cooldownUntil)}
                        </span>
                      ) : null}
                    </div>
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-2">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => onToggleRoute(route)}
                      >
                        {route.enabled ? "Disable" : "Enable"}
                      </Button>
                      <Button
                        type="button"
                        variant="destructive"
                        size="icon-sm"
                        onClick={() => onDeleteRoute(route)}
                      >
                        <Trash2Icon />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
              {routes.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={8}>
                    <EmptyNotice
                      title="No provider routes"
                      body="Add a route to make a catalog model reachable."
                    />
                  </TableCell>
                </TableRow>
              ) : null}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function ClientSetupView({
  accounts,
  models,
  routes,
  apiKey,
  setApiKey,
}: {
  accounts: ProviderAccount[];
  models: ModelCatalogEntry[];
  routes: ProviderModelRoute[];
  apiKey: string;
  setApiKey: React.Dispatch<React.SetStateAction<string>>;
}) {
  const serviceOrigin = useMemo(
    () => (typeof window === "undefined" ? "http://127.0.0.1:3000" : window.location.origin),
    [],
  );
  const catalogModels = useMemo(() => catalogModelIds(models), [models]);
  const catalogModelOptions = useMemo(() => enabledCatalogModelOptions(models), [models]);
  const codexModels = useMemo(
    () => routableModelsForWireApis(models, routes, accounts, ["openai-responses"]),
    [accounts, models, routes],
  );
  const claudeModels = useMemo(
    () => routableModelsForWireApis(models, routes, accounts, ["anthropic-messages"]),
    [accounts, models, routes],
  );
  const opencodeModels = useMemo(
    () => routableModelsForWireApis(models, routes, accounts, ["openai-chat"]),
    [accounts, models, routes],
  );
  const [codexModel, setCodexModel] = useState("");
  const [claudeModel, setClaudeModel] = useState("");
  const [opencodeModel, setOpencodeModel] = useState("");

  useEffect(() => {
    setCodexModel((current) => preferredCatalogModel(current, catalogModels, "gpt-5"));
  }, [catalogModels]);

  useEffect(() => {
    setClaudeModel((current) => preferredCatalogModel(current, catalogModels, "claude-sonnet-4-5"));
  }, [catalogModels]);

  useEffect(() => {
    setOpencodeModel((current) => preferredCatalogModel(current, catalogModels, "deepseek-v4"));
  }, [catalogModels]);

  const snippets = useMemo(
    () =>
      buildClientSetupSnippets({
        apiKey,
        serviceOrigin,
        codexModel,
        claudeModel,
        opencodeModel,
        opencodeModels: catalogModelOptions,
      }),
    [apiKey, serviceOrigin, codexModel, claudeModel, opencodeModel, catalogModelOptions],
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
              <SettingRow label="Chat routed" value={String(opencodeModels.length)} />
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
                value={opencodeModel}
                onChange={setOpencodeModel}
                options={catalogModels}
                routedOptions={opencodeModels}
                routeLabel="Chat"
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
            <ClientSnippetCard
              title="opencode project config"
              description="Creates an OpenAI-compatible provider backed by the chat namespace."
              endpoint="/openai/v1/chat/completions"
              model={opencodeModel}
              snippet={snippets.opencode}
            />
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

function RequestLogsView({
  logs,
  compact = false,
}: {
  logs: readonly RequestLog[];
  compact?: boolean;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{compact ? "Recent requests" : "Request Log"}</CardTitle>
        <CardDescription>
          Relay status, latency, model, sanitized routing metadata, and token accounting.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="hidden md:block">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Time</TableHead>
                <TableHead>Path</TableHead>
                <TableHead>Model</TableHead>
                <TableHead>Request</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Latency</TableHead>
                <TableHead>Tokens</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {logs.map((log) => (
                <TableRow key={log.id}>
                  <TableCell>{formatDate(log.createdAt)}</TableCell>
                  <TableCell>
                    <div className="flex max-w-[280px] flex-col gap-1">
                      <span className="truncate font-mono text-xs">{log.path}</span>
                      {log.upstreamUrl ? (
                        <span className="truncate font-mono text-xs text-muted-foreground">
                          {log.upstreamUrl}
                        </span>
                      ) : null}
                    </div>
                  </TableCell>
                  <TableCell>{formatLogModel(log)}</TableCell>
                  <TableCell className="max-w-[320px] text-xs text-muted-foreground">
                    {formatRequestSummary(log)}
                  </TableCell>
                  <TableCell>{statusCodeBadge(log.statusCode)}</TableCell>
                  <TableCell>{log.latencyMs}ms</TableCell>
                  <TableCell>{formatNumber(log.inputTokens + log.outputTokens)}</TableCell>
                </TableRow>
              ))}
              {logs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={7}>
                    <EmptyNotice
                      title="No relay traffic"
                      body="Requests appear here after clients call the relay."
                    />
                  </TableCell>
                </TableRow>
              ) : null}
            </TableBody>
          </Table>
        </div>
        <div className="grid gap-3 md:hidden">
          {logs.length === 0 ? (
            <EmptyNotice
              title="No relay traffic"
              body="Requests appear here after clients call the relay."
            />
          ) : (
            logs.map((log) => (
              <div key={log.id} className="flex flex-col gap-2 rounded-md border p-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate font-mono text-xs">{log.path}</div>
                    <div className="truncate text-xs text-muted-foreground">
                      {formatLogModel(log)}
                    </div>
                    {log.upstreamUrl ? (
                      <div className="truncate font-mono text-xs text-muted-foreground">
                        {log.upstreamUrl}
                      </div>
                    ) : null}
                  </div>
                  {statusCodeBadge(log.statusCode)}
                </div>
                <div className="grid grid-cols-3 gap-2 text-xs text-muted-foreground">
                  <span>{formatDate(log.createdAt)}</span>
                  <span>{log.latencyMs}ms</span>
                  <span>{formatNumber(log.inputTokens + log.outputTokens)} tokens</span>
                </div>
                <div className="text-xs text-muted-foreground">{formatRequestSummary(log)}</div>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function SettingsView() {
  return (
    <Tabs defaultValue="runtime">
      <TabsList>
        <TabsTrigger value="runtime">Runtime</TabsTrigger>
        <TabsTrigger value="headers">Headers</TabsTrigger>
      </TabsList>
      <TabsContent value="runtime">
        <Card>
          <CardHeader>
            <CardTitle>Runtime</CardTitle>
            <CardDescription>Current service assumptions exposed by the frontend.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4 md:grid-cols-2">
            <SettingRow label="Anthropic Messages" value="/anthropic/v1/messages" />
            <SettingRow label="Codex Responses" value="/openai/v1/responses" />
            <SettingRow label="OpenAI Chat" value="/openai/v1/chat/completions" />
            <SettingRow
              label="Gemini GenerateContent"
              value="/gemini/v1beta/models/{model}:generateContent"
            />
            <SettingRow label="Admin API" value="/admin/api" />
            <SettingRow label="Storage" value="SQLite" />
          </CardContent>
        </Card>
      </TabsContent>
      <TabsContent value="headers">
        <Card>
          <CardHeader>
            <CardTitle>Forwarded headers</CardTitle>
            <CardDescription>Headers preserved or supplied by the Rust relay.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {[
              "x-api-key",
              "x-goog-api-key",
              "authorization: Bearer",
              "anthropic-version",
              "anthropic-beta",
            ].map((item) => (
              <div key={item} className="rounded-md border p-3 font-mono text-sm">
                {item}
              </div>
            ))}
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>
  );
}

function SettingRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-w-0 flex-wrap items-center justify-between gap-2 rounded-md border p-3">
      <span className="shrink-0 text-sm text-muted-foreground">{label}</span>
      <span className="min-w-0 break-words text-right font-mono text-sm">{value}</span>
    </div>
  );
}

function CreateKeySheet({
  open,
  form,
  setForm,
  onOpenChange,
  onSubmit,
}: {
  open: boolean;
  form: CreateKeyForm;
  setForm: React.Dispatch<React.SetStateAction<CreateKeyForm>>;
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="sm:max-w-md">
        <SheetHeader>
          <SheetTitle>Create API key</SheetTitle>
          <SheetDescription>Issue a client key with optional routing limits.</SheetDescription>
        </SheetHeader>
        <form className="flex flex-col gap-4 px-4" onSubmit={onSubmit}>
          <Field label="Name" htmlFor="key-name">
            <Input
              id="key-name"
              value={form.name}
              onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))}
              required
            />
          </Field>
          <Field label="Description" htmlFor="key-description">
            <Textarea
              id="key-description"
              value={form.description}
              onChange={(event) =>
                setForm((current) => ({ ...current, description: event.target.value }))
              }
            />
          </Field>
          <Field label="Permissions" htmlFor="key-permissions">
            <Select
              value={form.permissions}
              onValueChange={(value) => setForm((current) => ({ ...current, permissions: value }))}
            >
              <SelectTrigger id="key-permissions">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="all">All services</SelectItem>
                  <SelectItem value="claude">Claude only</SelectItem>
                  <SelectItem value="openai">OpenAI-compatible only</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="RPM" htmlFor="key-rpm">
              <Input
                id="key-rpm"
                inputMode="numeric"
                value={form.rateLimitPerMinute}
                onChange={(event) =>
                  setForm((current) => ({ ...current, rateLimitPerMinute: event.target.value }))
                }
              />
            </Field>
            <Field label="Concurrency" htmlFor="key-concurrency">
              <Input
                id="key-concurrency"
                inputMode="numeric"
                value={form.concurrencyLimit}
                onChange={(event) =>
                  setForm((current) => ({ ...current, concurrencyLimit: event.target.value }))
                }
              />
            </Field>
            <Field label="Daily USD" htmlFor="key-cost">
              <Input
                id="key-cost"
                inputMode="decimal"
                value={form.dailyCostLimit}
                onChange={(event) =>
                  setForm((current) => ({ ...current, dailyCostLimit: event.target.value }))
                }
              />
            </Field>
          </div>
          <SheetFooter>
            <Button type="submit">
              <KeyRoundIcon data-icon="inline-start" />
              Create key
            </Button>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

function CreateAccountSheet({
  open,
  form,
  setForm,
  presets,
  onOpenChange,
  onSubmit,
}: {
  open: boolean;
  form: CreateAccountForm;
  setForm: React.Dispatch<React.SetStateAction<CreateAccountForm>>;
  presets: ProviderPreset[];
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}) {
  const isCodexSubscription = isCodexSubscriptionAuth(form.authMode);
  const isAntigravityAccount = isAntigravityAccountAuth(form.authMode);
  const usesTextareaCredential = isCodexSubscription;
  const selectedPreset = providerPresetForForm(form, presets);
  const credentialLabel =
    selectedPreset?.credentialLabel ??
    (isCodexSubscription ? "Raw refresh token" : "Upstream API key");
  const credentialPlaceholder =
    selectedPreset?.credentialPlaceholder ??
    (isCodexSubscription
      ? "Paste the value from tokens.refresh_token or openai.refresh"
      : "Upstream credential");
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="overflow-y-auto sm:max-w-lg">
        <SheetHeader>
          <SheetTitle>Add provider account</SheetTitle>
          <SheetDescription>Register an upstream credential for relay scheduling.</SheetDescription>
        </SheetHeader>
        <form className="flex flex-col gap-4 px-4" onSubmit={onSubmit}>
          <Field label="Preset" htmlFor="account-preset">
            <Select
              value={accountPresetValue(form, presets)}
              onValueChange={(value) => {
                if (value !== "custom") {
                  applyAccountPreset(value, presets, setForm);
                }
              }}
            >
              <SelectTrigger id="account-preset">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {presets.map((preset) => (
                    <SelectItem key={preset.id} value={preset.id}>
                      {preset.label}
                    </SelectItem>
                  ))}
                  <SelectItem value="custom">Custom</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field label="Name" htmlFor="account-name">
            <Input
              id="account-name"
              value={form.name}
              onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))}
              required
            />
          </Field>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Provider" htmlFor="account-provider">
              <Input
                id="account-provider"
                value={form.provider}
                onChange={(event) =>
                  setForm((current) => ({ ...current, provider: event.target.value }))
                }
              />
            </Field>
            <Field label="Protocol" htmlFor="account-wire-api">
              <Select
                value={form.wireApi}
                onValueChange={(value) => setForm((current) => ({ ...current, wireApi: value }))}
              >
                <SelectTrigger id="account-wire-api">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="anthropic-messages">Anthropic Messages</SelectItem>
                    <SelectItem value="openai-responses">OpenAI Responses</SelectItem>
                    <SelectItem value="openai-chat">OpenAI Chat</SelectItem>
                    <SelectItem value="gemini-generate-content">Gemini GenerateContent</SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Auth mode" htmlFor="account-auth-mode">
              <Select
                value={form.authMode}
                onValueChange={(value) => setForm((current) => ({ ...current, authMode: value }))}
              >
                <SelectTrigger id="account-auth-mode">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="x-api-key">x-api-key</SelectItem>
                    <SelectItem value="x-goog-api-key">x-goog-api-key</SelectItem>
                    <SelectItem value="bearer">Bearer</SelectItem>
                    <SelectItem value="codex-oauth">Codex OAuth</SelectItem>
                    <SelectItem value="antigravity-oauth">Antigravity OAuth</SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <SettingRow label="Route binding" value="Configured in Model Catalog" />
          </div>
          {selectedPreset?.credentialHelp ? (
            <Alert>
              <KeyRoundIcon className="size-4" />
              <AlertTitle>{selectedPreset.label} credential</AlertTitle>
              <AlertDescription>{selectedPreset.credentialHelp}</AlertDescription>
            </Alert>
          ) : null}
          <Field
            label={
              isCodexSubscription
                ? "Codex account API base"
                : isAntigravityAccount
                  ? "Gemini endpoint base"
                  : "Base URL"
            }
            htmlFor="account-base-url"
          >
            <Input
              id="account-base-url"
              value={form.baseUrl}
              onChange={(event) =>
                setForm((current) => ({ ...current, baseUrl: event.target.value }))
              }
              required
            />
          </Field>
          {!isAntigravityAccount ? (
            <Field label={credentialLabel} htmlFor="account-api-key">
              {usesTextareaCredential ? (
                <Textarea
                  id="account-api-key"
                  className="min-h-28 font-mono text-xs"
                  value={form.apiKey}
                  onChange={(event) =>
                    setForm((current) => ({ ...current, apiKey: event.target.value }))
                  }
                  placeholder={credentialPlaceholder}
                  required
                />
              ) : (
                <Input
                  id="account-api-key"
                  type="password"
                  value={form.apiKey}
                  onChange={(event) =>
                    setForm((current) => ({ ...current, apiKey: event.target.value }))
                  }
                  placeholder={credentialPlaceholder}
                  required
                />
              )}
            </Field>
          ) : null}
          <div className="grid gap-3 sm:grid-cols-[1fr_120px]">
            <SettingRow
              label="Upstream path"
              value={upstreamPathForWireApi(form.wireApi, form.authMode)}
            />
            <Field label="Priority" htmlFor="account-priority">
              <Input
                id="account-priority"
                inputMode="numeric"
                value={form.priority}
                onChange={(event) =>
                  setForm((current) => ({ ...current, priority: event.target.value }))
                }
              />
            </Field>
          </div>
          <div className="flex items-center justify-between gap-3 rounded-md border p-3">
            <div className="flex flex-col gap-1">
              <Label htmlFor="account-active">Schedulable</Label>
              <span className="text-xs text-muted-foreground">
                Use this account for relay traffic
              </span>
            </div>
            <Switch
              id="account-active"
              checked={form.isActive}
              onCheckedChange={(checked) =>
                setForm((current) => ({ ...current, isActive: checked }))
              }
            />
          </div>
          <SheetFooter>
            <Button type="submit">
              {isAntigravityAccount ? (
                <LogInIcon data-icon="inline-start" />
              ) : (
                <CableIcon data-icon="inline-start" />
              )}
              {isAntigravityAccount ? "Sign in with Antigravity" : "Add account"}
            </Button>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

function CreateModelSheet({
  open,
  form,
  setForm,
  onOpenChange,
  onSubmit,
}: {
  open: boolean;
  form: ModelCatalogForm;
  setForm: React.Dispatch<React.SetStateAction<ModelCatalogForm>>;
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="sm:max-w-md">
        <SheetHeader>
          <SheetTitle>Add catalog model</SheetTitle>
          <SheetDescription>Register an exact public model name clients may send.</SheetDescription>
        </SheetHeader>
        <form className="flex flex-col gap-4 px-4" onSubmit={onSubmit}>
          <Field label="Model ID" htmlFor="model-id">
            <Input
              id="model-id"
              value={form.id}
              onChange={(event) => setForm((current) => ({ ...current, id: event.target.value }))}
              placeholder="MiniMax-M3"
              required
            />
          </Field>
          <Field label="Display name" htmlFor="model-display-name">
            <Input
              id="model-display-name"
              value={form.displayName}
              onChange={(event) =>
                setForm((current) => ({ ...current, displayName: event.target.value }))
              }
              placeholder="Defaults to model ID"
            />
          </Field>
          <Field label="Family" htmlFor="model-family">
            <Input
              id="model-family"
              value={form.family}
              onChange={(event) =>
                setForm((current) => ({ ...current, family: event.target.value }))
              }
              placeholder="minimax, deepseek, glm, openai, anthropic"
            />
          </Field>
          <div className="flex items-center justify-between gap-3 rounded-md border p-3">
            <div className="flex flex-col gap-1">
              <Label htmlFor="model-enabled">Advertise when routed</Label>
              <span className="text-xs text-muted-foreground">
                Disabled models are hidden from client model lists.
              </span>
            </div>
            <Switch
              id="model-enabled"
              checked={form.enabled}
              onCheckedChange={(checked) =>
                setForm((current) => ({ ...current, enabled: checked }))
              }
            />
          </div>
          <SheetFooter>
            <Button type="submit">
              <DatabaseIcon data-icon="inline-start" />
              Add model
            </Button>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

function CreateRouteSheet({
  open,
  form,
  setForm,
  models,
  accounts,
  onOpenChange,
  onSubmit,
}: {
  open: boolean;
  form: ProviderRouteForm;
  setForm: React.Dispatch<React.SetStateAction<ProviderRouteForm>>;
  models: ModelCatalogEntry[];
  accounts: ProviderAccount[];
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="overflow-y-auto sm:max-w-lg">
        <SheetHeader>
          <SheetTitle>Add provider model route</SheetTitle>
          <SheetDescription>Bind a public model to an upstream provider model.</SheetDescription>
        </SheetHeader>
        <form className="flex flex-col gap-4 px-4" onSubmit={onSubmit}>
          <Field label="Public model" htmlFor="route-public-model">
            <Select
              value={form.publicModelId || "__none"}
              onValueChange={(value) =>
                setForm((current) => ({
                  ...current,
                  publicModelId: value === "__none" ? "" : value,
                  upstreamModelId: current.upstreamModelId || (value === "__none" ? "" : value),
                }))
              }
            >
              <SelectTrigger id="route-public-model">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="__none" disabled>
                    Select model
                  </SelectItem>
                  {models.map((model) => (
                    <SelectItem key={model.id} value={model.id}>
                      {model.id}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field label="Provider account" htmlFor="route-provider-account">
            <Select
              value={form.providerAccountId || "__none"}
              onValueChange={(value) =>
                setForm((current) => ({
                  ...current,
                  providerAccountId: value === "__none" ? "" : value,
                }))
              }
            >
              <SelectTrigger id="route-provider-account">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="__none" disabled>
                    Select account
                  </SelectItem>
                  {accounts.map((account) => (
                    <SelectItem key={account.id} value={account.id}>
                      {account.name}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field label="Upstream model ID" htmlFor="route-upstream-model">
            <Input
              id="route-upstream-model"
              value={form.upstreamModelId}
              onChange={(event) =>
                setForm((current) => ({ ...current, upstreamModelId: event.target.value }))
              }
              placeholder="Exact upstream model ID"
              required
            />
          </Field>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Protocol" htmlFor="route-wire-api">
              <Select
                value={form.wireApi}
                onValueChange={(value) => setForm((current) => ({ ...current, wireApi: value }))}
              >
                <SelectTrigger id="route-wire-api">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="anthropic-messages">Anthropic Messages</SelectItem>
                    <SelectItem value="openai-responses">OpenAI Responses</SelectItem>
                    <SelectItem value="openai-chat">OpenAI Chat</SelectItem>
                    <SelectItem value="gemini-generate-content">Gemini GenerateContent</SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <Field label="Role" htmlFor="route-role">
              <Select
                value={form.role}
                onValueChange={(value) => setForm((current) => ({ ...current, role: value }))}
              >
                <SelectTrigger id="route-role">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="primary">Primary</SelectItem>
                    <SelectItem value="backup">Backup</SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          </div>
          <Field label="Strip request params" htmlFor="route-strip-params">
            <Input
              id="route-strip-params"
              value={form.stripParams}
              onChange={(event) =>
                setForm((current) => ({ ...current, stripParams: event.target.value }))
              }
              placeholder="temperature, top_p"
            />
          </Field>
          <div className="flex items-center justify-between gap-3 rounded-md border p-3">
            <div className="flex flex-col gap-1">
              <Label htmlFor="route-enabled">Enabled</Label>
              <span className="text-xs text-muted-foreground">
                Enabled primary routes must be unique for a model and protocol.
              </span>
            </div>
            <Switch
              id="route-enabled"
              checked={form.enabled}
              onCheckedChange={(checked) =>
                setForm((current) => ({ ...current, enabled: checked }))
              }
            />
          </div>
          <SheetFooter>
            <Button type="submit">
              <RouteIcon data-icon="inline-start" />
              Add route
            </Button>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
    </div>
  );
}

function LoadingState() {
  return (
    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      {Array.from({ length: 4 }).map((_, index) => (
        <Card key={index}>
          <CardHeader>
            <Skeleton className="h-4 w-28" />
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <Skeleton className="h-8 w-20" />
            <Skeleton className="h-3 w-32" />
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

function EmptyNotice({ title, body }: { title: string; body: string }) {
  return (
    <div className="flex min-h-28 flex-col items-center justify-center gap-2 rounded-md border border-dashed p-6 text-center">
      <div className="text-sm font-medium">{title}</div>
      <div className="text-sm text-muted-foreground">{body}</div>
    </div>
  );
}

function TrendChart({ values }: { values: number[] }) {
  const peak = Math.max(...values, 0);
  const max = Math.max(peak, 1);
  const points = values
    .map((value, index) => {
      const x = (index / Math.max(values.length - 1, 1)) * 100;
      const y = 100 - (value / max) * 78 - 10;
      return `${x},${y}`;
    })
    .join(" ");
  const total = values.reduce((sum, value) => sum + value, 0);

  return (
    <div className="flex flex-col gap-4">
      <div className="h-60 rounded-lg border bg-muted/30 p-4">
        <svg viewBox="0 0 100 100" preserveAspectRatio="none" className="h-full w-full">
          <polyline points={points} fill="none" stroke="currentColor" strokeWidth="2" />
          <polygon points={`0,100 ${points} 100,100`} className="fill-primary/10" />
        </svg>
      </div>
      <div className="grid gap-3 md:grid-cols-3">
        <ChartStat label="Window total" value={formatNumber(total)} />
        <ChartStat label="Peak bucket" value={formatNumber(peak)} />
        <ChartStat label="Buckets" value={String(values.length)} />
      </div>
    </div>
  );
}

function ChartStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1 rounded-md border p-3">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-sm font-semibold">{value}</span>
    </div>
  );
}

function statusBadge(status: string, active: boolean) {
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

function routeRoleBadge(role: string) {
  if (role === "primary") {
    return <Badge variant="secondary">primary</Badge>;
  }
  return <Badge variant="outline">{role}</Badge>;
}

function routeCountForAccount(routes: ProviderModelRoute[], accountId: string) {
  return routes.filter((route) => route.providerAccountId === accountId).length;
}

function routeCountForModel(routes: ProviderModelRoute[], modelId: string) {
  return routes.filter((route) => route.publicModelId === modelId).length;
}

function accountName(accounts: ProviderAccount[], accountId: string) {
  return accounts.find((account) => account.id === accountId)?.name ?? accountId;
}

function uniqueSorted(values: string[]) {
  return Array.from(new Set(values)).sort((left, right) => left.localeCompare(right));
}

function commaSeparatedValues(value: string) {
  return uniqueSorted(
    value
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean),
  );
}

function formatRoutePolicy(route: ProviderModelRoute) {
  if (route.stripParams.length === 0) {
    return "No stripped params";
  }
  return `strip ${route.stripParams.join(", ")}`;
}

function statusCodeBadge(status: number) {
  if (status >= 200 && status < 300) {
    return <Badge variant="secondary">{status}</Badge>;
  }
  if (status === 429 || status >= 500) {
    return <Badge variant="destructive">{status}</Badge>;
  }
  return <Badge variant="outline">{status}</Badge>;
}

function formatLogModel(log: RequestLog) {
  const publicModel = log.model || "unknown";
  if (!log.upstreamModel || log.upstreamModel === publicModel) {
    return publicModel;
  }
  return `${publicModel} -> ${log.upstreamModel}`;
}

function formatRequestSummary(log: RequestLog) {
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

function buildTrend(logs: readonly RequestLog[]) {
  const buckets = new Array<number>(12).fill(0);
  logs.forEach((_, index) => {
    buckets[index % buckets.length] += 1;
  });
  return buckets.reverse();
}

function currentViewLabel(view: View) {
  return views.find((item) => item.id === view)?.label ?? "Overview";
}

function providerPresetForForm(form: CreateAccountForm, presets: ProviderPreset[]) {
  return presets.find(
    (preset) =>
      preset.provider === form.provider &&
      preset.baseUrl === form.baseUrl &&
      preset.authMode === form.authMode &&
      preset.wireApi === form.wireApi,
  );
}

function accountPresetValue(form: CreateAccountForm, presets: ProviderPreset[]) {
  return providerPresetForForm(form, presets)?.id ?? "custom";
}

function applyAccountPreset(
  presetId: string,
  presets: ProviderPreset[],
  setForm: React.Dispatch<React.SetStateAction<CreateAccountForm>>,
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

function routableModelsForWireApis(
  models: ModelCatalogEntry[],
  routes: ProviderModelRoute[],
  accounts: ProviderAccount[],
  wireApis: string[],
) {
  const acceptedWireApis = new Set(wireApis);
  const activeAccounts = new Set(
    accounts
      .filter((account) => account.isActive && account.status !== "blocked")
      .map((account) => account.id),
  );
  const routableIds = new Set(
    routes
      .filter(
        (route) =>
          routeIsEligible(route) &&
          acceptedWireApis.has(route.wireApi) &&
          activeAccounts.has(route.providerAccountId),
      )
      .map((route) => route.publicModelId),
  );
  return uniqueSorted(
    models.filter((model) => model.enabled && routableIds.has(model.id)).map((model) => model.id),
  );
}

function routeIsEligible(route: ProviderModelRoute) {
  return (
    route.enabled &&
    route.status !== "blocked" &&
    (!route.cooldownUntil || new Date(route.cooldownUntil).getTime() <= Date.now())
  );
}

function catalogModelIds(models: ModelCatalogEntry[]) {
  return uniqueSorted(models.filter((model) => model.enabled).map((model) => model.id));
}

function enabledCatalogModelOptions(models: ModelCatalogEntry[]): ClientModelOption[] {
  return models
    .filter((model) => model.enabled && model.id)
    .map((model) => ({
      id: model.id,
      displayName: model.displayName || model.id,
    }))
    .sort((left, right) => left.id.localeCompare(right.id));
}

function uniqueModelOptions(models: ClientModelOption[]) {
  const byId = new Map<string, ClientModelOption>();
  models.forEach((model) => {
    const id = model.id.trim();
    if (!id || byId.has(id)) {
      return;
    }
    byId.set(id, {
      id,
      displayName: model.displayName.trim() || id,
    });
  });
  return Array.from(byId.values()).sort((left, right) => left.id.localeCompare(right.id));
}

function preferredCatalogModel(current: string, catalogModels: string[], fallback: string) {
  if (catalogModels.length === 0) {
    return current || fallback;
  }
  return current && catalogModels.includes(current) ? current : catalogModels[0];
}

function buildClientSetupSnippets({
  apiKey,
  serviceOrigin,
  codexModel,
  claudeModel,
  opencodeModel,
  opencodeModels,
}: {
  apiKey: string;
  serviceOrigin: string;
  codexModel: string;
  claudeModel: string;
  opencodeModel: string;
  opencodeModels: ClientModelOption[];
}) {
  const origin = serviceOrigin.replace(/\/+$/, "");
  const relayApiKey = apiKey.trim() || "tokentoxication-REPLACE_ME";
  const openaiBaseUrl = `${origin}/openai/v1`;
  const anthropicBaseUrl = `${origin}/anthropic`;
  const codexModelName = codexModel.trim() || "gpt-5";
  const claudeModelName = claudeModel.trim() || "claude-sonnet-4-5";
  const opencodeModelName = opencodeModel.trim() || "deepseek-v4";
  const opencodeModelEntries = uniqueModelOptions([
    {
      id: opencodeModelName,
      displayName:
        opencodeModels.find((model) => model.id === opencodeModelName)?.displayName ||
        opencodeModelName,
    },
    ...opencodeModels,
  ]);
  const opencodeProvider = "token-toxication";
  const opencodeConfig = JSON.stringify(
    {
      $schema: "https://opencode.ai/config.json",
      provider: {
        [opencodeProvider]: {
          npm: "@ai-sdk/openai-compatible",
          name: "Token Toxication",
          options: {
            baseURL: openaiBaseUrl,
            apiKey: "{env:TOKEN_TOXICATION_API_KEY}",
          },
          models: Object.fromEntries(
            opencodeModelEntries.map((model) => [
              model.id,
              {
                name: model.displayName,
              },
            ]),
          ),
        },
      },
      model: `${opencodeProvider}/${opencodeModelName}`,
      small_model: `${opencodeProvider}/${opencodeModelName}`,
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
  };
}

function shellQuote(value: string) {
  return `'${value.replace(/'/g, "'\\''")}'`;
}

function tomlString(value: string) {
  return JSON.stringify(value);
}

function wireApiLabel(value: string) {
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

function upstreamPathForWireApi(value: string, authMode?: string) {
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

function isCodexSubscriptionAuth(value: string) {
  return value === "codex-oauth";
}

function isAntigravityAccountAuth(value: string) {
  return value === "antigravity-oauth";
}

function isGeminiAccount(account: ProviderAccount) {
  return account.provider === "gemini" && isAntigravityAccountAuth(account.authMode);
}

function isCodexAccount(account: ProviderAccount) {
  return isCodexSubscriptionAuth(account.authMode);
}

function numberFromInput(value: string) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat().format(value);
}

function formatGeminiTier(tier: GeminiAccountQuotaResponse["currentTier"]) {
  if (!tier) {
    return "unknown";
  }
  return tier.name || tier.id || "unknown";
}

function quotaPercent(value: number | null | undefined) {
  return value == null ? undefined : Math.max(0, Math.min(100, value * 100));
}

function formatQuotaPercent(value: number) {
  if (value === 100) {
    return "100%";
  }
  return `${value.toFixed(value >= 99 ? 3 : 1)}%`;
}

function codexUsedPercent(value: number | null | undefined) {
  return value == null || !Number.isFinite(value) ? undefined : Math.max(0, Math.min(100, value));
}

function formatCodexWindow(seconds: number | null | undefined) {
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

function formatQuotaReset(value: string | null | undefined) {
  return value ? formatDate(value) : "unknown";
}

function codexQuotaStatus(
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

function formatCodexCredits(credits: NonNullable<CodexAccountQuotaResponse["credits"]>) {
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

function formatCodexSpendControl(
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

function formatOptionalNumber(value: number | null | undefined) {
  return value == null ? "unknown" : formatNumber(value);
}

function humanizeIdentifier(value: string) {
  const text = value.replaceAll("_", " ").trim();
  return text ? `${text.charAt(0).toUpperCase()}${text.slice(1)}` : "Unknown";
}

function formatDate(value: string | null | undefined) {
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

async function copyText(value: string) {
  await navigator.clipboard.writeText(value);
  toast.success("Copied");
}

export default App;
