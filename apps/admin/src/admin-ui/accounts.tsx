import { useMemo } from "react";
import type React from "react";
import { Link, useNavigate, useParams } from "react-router";
import {
  ActivityIcon,
  CableIcon,
  GaugeIcon,
  KeyRoundIcon,
  LogInIcon,
  PencilIcon,
  PlusIcon,
  ShieldCheckIcon,
  Trash2Icon,
} from "lucide-react";

import { adminPaths } from "../admin-routes";
import {
  codexQuotaStatus,
  codexUsedPercent,
  accountPresetValue,
  applyAccountPreset,
  formatCodexCredits,
  formatCodexSpendControl,
  formatCodexWindow,
  formatDate,
  formatGeminiTier,
  formatOptionalNumber,
  formatQuotaPercent,
  formatQuotaReset,
  humanizeIdentifier,
  isCodexAccount,
  isGeminiAccount,
  isAntigravityAccountAuth,
  isCodexSubscriptionAuth,
  providerPresetForForm,
  quotaPercent,
  routeCountForAccount,
  routeRoleBadge,
  statusBadge,
  upstreamPathForWireApi,
  wireApiLabel,
} from "./helpers";
import { EmptyNotice, Field, MissingRecordView, SettingRow } from "./shared";
import type { CodexQuotaRow, CreateAccountForm } from "./types";
import type {
  CodexAccountQuotaResponse,
  CodexAccountQuotaWindow,
  GeminiAccountModelsResponse,
  GeminiAccountQuotaResponse,
  ProviderAccount,
  ProviderModelRoute,
  ProviderPreset,
} from "../types";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
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
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Textarea } from "@/components/ui/textarea";

export function AccountsView({
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
                  <TableCell className="font-medium">
                    <Link
                      to={adminPaths.account(account.id)}
                      className="underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      {account.name}
                    </Link>
                  </TableCell>
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
                    <Link
                      to={adminPaths.account(account.id)}
                      className="block truncate text-sm font-medium underline-offset-4 hover:underline"
                    >
                      {account.name}
                    </Link>
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

export function AccountDetailView({
  accounts,
  routes,
  onToggle,
  onDelete,
  onEdit,
  onInspectCodex,
  onInspectGemini,
  onReconnectAntigravity,
}: {
  accounts: readonly ProviderAccount[];
  routes: readonly ProviderModelRoute[];
  onToggle: (account: ProviderAccount) => void;
  onDelete: (account: ProviderAccount) => Promise<boolean>;
  onEdit: (account: ProviderAccount) => void;
  onInspectCodex: (account: ProviderAccount) => void;
  onInspectGemini: (account: ProviderAccount) => void;
  onReconnectAntigravity: (account: ProviderAccount) => void;
}) {
  const { accountId } = useParams();
  const navigate = useNavigate();
  const account = accounts.find((item) => item.id === accountId);

  if (!account) {
    return (
      <MissingRecordView
        title="Provider account not found"
        body="This provider account may have been deleted or the link is incomplete."
        to={adminPaths.accounts()}
        label="Back to provider accounts"
      />
    );
  }

  const selectedAccount = account;
  const linkedRoutes = routes.filter((route) => route.providerAccountId === account.id);

  async function handleDelete() {
    if (await onDelete(selectedAccount)) {
      void navigate(adminPaths.accounts());
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
        <div className="flex min-w-0 flex-col gap-2">
          <Button asChild type="button" variant="ghost" size="sm" className="w-fit">
            <Link to={adminPaths.accounts()}>Provider Accounts</Link>
          </Button>
          <div>
            <div className="text-sm text-muted-foreground">Provider Account</div>
            <h1 className="truncate text-2xl font-semibold">{account.name}</h1>
            <p className="mt-1 truncate text-sm text-muted-foreground">{account.baseUrl}</p>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          {isCodexAccount(account) ? (
            <Button type="button" variant="outline" onClick={() => onInspectCodex(account)}>
              <GaugeIcon data-icon="inline-start" />
              Quota
            </Button>
          ) : null}
          {isGeminiAccount(account) ? (
            <Button type="button" variant="outline" onClick={() => onInspectGemini(account)}>
              <GaugeIcon data-icon="inline-start" />
              Models and quota
            </Button>
          ) : null}
          {isGeminiAccount(account) ? (
            <Button type="button" variant="outline" onClick={() => onReconnectAntigravity(account)}>
              <LogInIcon data-icon="inline-start" />
              Reconnect
            </Button>
          ) : null}
          <Button type="button" variant="outline" onClick={() => onToggle(account)}>
            {account.isActive ? "Disable" : "Enable"}
          </Button>
          <Button type="button" variant="outline" onClick={() => onEdit(account)}>
            <PencilIcon data-icon="inline-start" />
            Edit
          </Button>
          <Button type="button" variant="destructive" onClick={handleDelete}>
            <Trash2Icon data-icon="inline-start" />
            Delete
          </Button>
        </div>
      </div>

      <div className="grid gap-5 xl:grid-cols-[0.75fr_1.25fr]">
        <Card>
          <CardHeader>
            <CardTitle>Connection</CardTitle>
            <CardDescription>Configured upstream identity and scheduling state.</CardDescription>
          </CardHeader>
          <CardContent>
            <dl className="grid gap-4 text-sm">
              <div>
                <dt className="text-xs text-muted-foreground">Provider</dt>
                <dd className="mt-1">{account.provider}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">Protocol</dt>
                <dd className="mt-1">{wireApiLabel(account.wireApi)}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">Priority</dt>
                <dd className="mt-1">{account.priority}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">Status</dt>
                <dd className="mt-1">{statusBadge(account.status, account.isActive)}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">Created</dt>
                <dd className="mt-1">{formatDate(account.createdAt)}</dd>
              </div>
            </dl>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Provider Model Routes</CardTitle>
            <CardDescription>
              Catalog Model bindings that use this Provider Account.
            </CardDescription>
          </CardHeader>
          <CardContent>
            {linkedRoutes.length === 0 ? (
              <EmptyNotice
                title="No provider routes"
                body="Add a provider route from a Catalog Model to make this account schedulable."
              />
            ) : (
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Catalog Model</TableHead>
                      <TableHead>Upstream Model</TableHead>
                      <TableHead>Role</TableHead>
                      <TableHead>Status</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {linkedRoutes.map((route) => (
                      <TableRow key={route.id}>
                        <TableCell>
                          <Link
                            to={adminPaths.model(route.publicModelId)}
                            className="font-mono text-xs underline-offset-4 hover:underline"
                          >
                            {route.publicModelId}
                          </Link>
                        </TableCell>
                        <TableCell>
                          <Link
                            to={adminPaths.route(route.id)}
                            className="font-mono text-xs underline-offset-4 hover:underline"
                          >
                            {route.upstreamModelId}
                          </Link>
                        </TableCell>
                        <TableCell>{routeRoleBadge(route.role)}</TableCell>
                        <TableCell>{statusBadge(route.status, route.enabled)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

export function GeminiAccountDialog({
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

export function CodexAccountDialog({
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

export function CreateAccountSheet({
  open,
  form,
  setForm,
  presets,
  editing,
  onOpenChange,
  onSubmit,
}: {
  open: boolean;
  form: CreateAccountForm;
  setForm: React.Dispatch<React.SetStateAction<CreateAccountForm>>;
  presets: ProviderPreset[];
  editing: boolean;
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
          <SheetTitle>{editing ? "Edit provider account" : "Add provider account"}</SheetTitle>
          <SheetDescription>
            {editing
              ? "Update routing details or replace the upstream credential."
              : "Register an upstream credential for relay scheduling."}
          </SheetDescription>
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
                  required={!editing}
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
                  required={!editing}
                />
              )}
            </Field>
          ) : null}
          {editing && !isAntigravityAccount ? (
            <p className="text-xs text-muted-foreground">
              Leave the credential blank to keep the current value.
            </p>
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
              {isAntigravityAccount && !editing
                ? "Sign in with Antigravity"
                : editing
                  ? "Save changes"
                  : "Add account"}
            </Button>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}
