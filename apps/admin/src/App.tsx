import type { ComponentType } from "react";
import {
  ActivityIcon,
  CableIcon,
  CheckIcon,
  ClipboardCopyIcon,
  DatabaseIcon,
  KeyRoundIcon,
  LayoutDashboardIcon,
  LogOutIcon,
  RefreshCcwIcon,
  RouteIcon,
  SettingsIcon,
  ShieldCheckIcon,
  TerminalSquareIcon,
} from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { NavLink, useLocation, useNavigate } from "react-router";

import {
  AdminRoutes,
  adminNavigation,
  adminPathId,
  adminPathLabel,
  adminPaths,
  adminRouteLabel,
  adminRoutePath,
  type AdminRouteId,
} from "./admin-routes";
import { ClientSetupView } from "./admin-ui/client-setup";
import {
  AccountDetailView,
  AccountsView,
  CodexAccountDialog,
  CreateAccountSheet,
  GeminiAccountDialog,
} from "./admin-ui/accounts";
import { ApiKeyDetailView, ApiKeysView, CreateKeySheet } from "./admin-ui/api-keys";
import { Overview } from "./admin-ui/dashboard";
import { RequestLogDetailView, RequestLogsView } from "./admin-ui/logs";
import {
  CreateModelSheet,
  CreateRouteSheet,
  ModelCatalogView,
  ModelDetailView,
  ProviderRouteDetailView,
} from "./admin-ui/models";
import { EmptyNotice, LoadingState, NotFoundView } from "./admin-ui/shared";
import { SettingsView } from "./admin-ui/settings";
import { useAdminWorkspace } from "./admin-ui/use-admin-workspace";
import { copyText } from "./admin-ui/helpers";

const navigationIcons: Record<AdminRouteId, ComponentType<{ className?: string }>> = {
  overview: LayoutDashboardIcon,
  keys: KeyRoundIcon,
  "key-detail": KeyRoundIcon,
  accounts: CableIcon,
  "account-detail": CableIcon,
  models: DatabaseIcon,
  "model-detail": DatabaseIcon,
  "route-detail": DatabaseIcon,
  setup: TerminalSquareIcon,
  logs: ActivityIcon,
  "log-detail": ActivityIcon,
  settings: SettingsIcon,
  "not-found": RouteIcon,
};

function App() {
  const location = useLocation();
  const navigate = useNavigate();
  const {
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
  } = useAdminWorkspace();

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
                {adminNavigation.map((item) => {
                  const Icon = navigationIcons[item.id];
                  return (
                    <NavLink
                      key={item.id}
                      to={adminRoutePath(item.id)}
                      className={buttonVariants({
                        variant: item.activeIds.includes(adminPathId(location.pathname))
                          ? "secondary"
                          : "ghost",
                        className: "justify-start",
                      })}
                    >
                      <Icon data-icon="inline-start" />
                      {adminRouteLabel(item.id)}
                    </NavLink>
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
                    <span className="truncate text-sm font-medium">
                      {adminPathLabel(location.pathname)}
                    </span>
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
                <AdminRoutes
                  elements={{
                    overview: dashboard ? (
                      <Overview
                        dashboard={dashboard}
                        onCreateKey={() => setIsKeySheetOpen(true)}
                        onCreateAccount={openCreateAccount}
                      />
                    ) : (
                      <EmptyNotice
                        title="Dashboard unavailable"
                        body="Refresh the admin data to load the overview."
                      />
                    ),
                    keys: (
                      <ApiKeysView
                        apiKeys={apiKeys}
                        onCreate={() => setIsKeySheetOpen(true)}
                        onToggle={toggleApiKey}
                      />
                    ),
                    "key-detail": (
                      <ApiKeyDetailView
                        apiKeys={apiKeys}
                        onToggle={toggleApiKey}
                        onDelete={deleteApiKey}
                      />
                    ),
                    accounts: (
                      <AccountsView
                        accounts={accounts}
                        routes={modelRoutes}
                        onCreate={openCreateAccount}
                        onToggle={toggleAccount}
                        onInspectCodex={inspectCodexAccount}
                        onInspectGemini={inspectGeminiAccount}
                        onReconnectAntigravity={reconnectAntigravityAccount}
                      />
                    ),
                    "account-detail": (
                      <AccountDetailView
                        accounts={accounts}
                        routes={modelRoutes}
                        onToggle={toggleAccount}
                        onDelete={deleteAccount}
                        onEdit={openEditAccount}
                        onInspectCodex={inspectCodexAccount}
                        onInspectGemini={inspectGeminiAccount}
                        onReconnectAntigravity={reconnectAntigravityAccount}
                      />
                    ),
                    models: (
                      <ModelCatalogView
                        accounts={accounts}
                        models={modelCatalog}
                        routes={modelRoutes}
                        onCreateModel={() => setIsModelSheetOpen(true)}
                        onCreateRoute={() => setIsRouteSheetOpen(true)}
                      />
                    ),
                    "model-detail": (
                      <ModelDetailView
                        accounts={accounts}
                        models={modelCatalog}
                        routes={modelRoutes}
                        onToggle={toggleModel}
                        onUpdate={updateModelDetails}
                      />
                    ),
                    "route-detail": (
                      <ProviderRouteDetailView
                        accounts={accounts}
                        models={modelCatalog}
                        routes={modelRoutes}
                        onToggle={toggleRoute}
                        onDelete={deleteRoute}
                      />
                    ),
                    setup: (
                      <ClientSetupView
                        models={modelCatalog}
                        routableModels={routableModels}
                        apiKey={clientSetupApiKey}
                        setApiKey={setClientSetupApiKey}
                      />
                    ),
                    logs: <RequestLogsView logs={logs} />,
                    "log-detail": <RequestLogDetailView logs={logs} />,
                    settings: <SettingsView />,
                    "not-found": <NotFoundView />,
                  }}
                />
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
        editing={Boolean(editingAccount)}
        onOpenChange={handleAccountSheetOpenChange}
        onSubmit={handleSaveAccount}
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
                void navigate(adminPaths.setup());
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

export default App;
