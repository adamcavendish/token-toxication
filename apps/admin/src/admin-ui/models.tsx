import { useEffect, useMemo, useState } from "react";
import type React from "react";
import { Link, useNavigate, useParams } from "react-router";
import { DatabaseIcon, PlusIcon, RouteIcon, Trash2Icon } from "lucide-react";

import { adminPaths } from "../admin-routes";
import {
  accountName,
  formatDate,
  formatRoutePolicy,
  routeCountForModel,
  routeRoleBadge,
  statusBadge,
  wireApiLabel,
} from "./helpers";
import { EmptyNotice, Field, MissingRecordView } from "./shared";
import type { ModelCatalogForm, ProviderRouteForm } from "./types";
import type { ModelCatalogEntry, ProviderAccount, ProviderModelRoute } from "../types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export function ModelCatalogView({
  accounts,
  models,
  routes,
  onCreateModel,
  onCreateRoute,
}: {
  accounts: ProviderAccount[];
  models: ModelCatalogEntry[];
  routes: ProviderModelRoute[];
  onCreateModel: () => void;
  onCreateRoute: () => void;
}) {
  const [query, setQuery] = useState("");
  const [family, setFamily] = useState("all");
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
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredModels.map((model) => (
                  <TableRow key={model.id}>
                    <TableCell className="font-mono text-xs">
                      <Link
                        to={adminPaths.model(model.id)}
                        className="underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      >
                        {model.id}
                      </Link>
                    </TableCell>
                    <TableCell>{model.displayName}</TableCell>
                    <TableCell>{model.family}</TableCell>
                    <TableCell>{routeCountForModel(routes, model.id)}</TableCell>
                    <TableCell>
                      {model.enabled ? statusBadge("healthy", true) : statusBadge("paused", false)}
                    </TableCell>
                  </TableRow>
                ))}
                {filteredModels.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={5}>
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
                      <Link
                        to={adminPaths.model(model.id)}
                        className="block truncate font-mono text-sm underline-offset-4 hover:underline"
                      >
                        {model.id}
                      </Link>
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
              </TableRow>
            </TableHeader>
            <TableBody>
              {routes.map((route) => (
                <TableRow key={route.id}>
                  <TableCell className="font-mono text-xs">
                    <Link
                      to={adminPaths.model(route.publicModelId)}
                      className="underline-offset-4 hover:underline"
                    >
                      {route.publicModelId}
                    </Link>
                  </TableCell>
                  <TableCell className="font-mono text-xs">
                    <Link
                      to={adminPaths.route(route.id)}
                      className="underline-offset-4 hover:underline"
                    >
                      {route.upstreamModelId}
                    </Link>
                  </TableCell>
                  <TableCell>
                    <Link
                      to={adminPaths.account(route.providerAccountId)}
                      className="underline-offset-4 hover:underline"
                    >
                      {accountName(accounts, route.providerAccountId)}
                    </Link>
                  </TableCell>
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
                </TableRow>
              ))}
              {routes.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={7}>
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

export function ModelDetailView({
  accounts,
  models,
  routes,
  onToggle,
  onUpdate,
}: {
  accounts: readonly ProviderAccount[];
  models: readonly ModelCatalogEntry[];
  routes: readonly ProviderModelRoute[];
  onToggle: (model: ModelCatalogEntry) => Promise<void>;
  onUpdate: (
    model: ModelCatalogEntry,
    values: { displayName: string; family: string },
  ) => Promise<void>;
}) {
  const { modelId } = useParams();
  const model = models.find((item) => item.id === modelId);
  const [draft, setDraft] = useState({ displayName: "", family: "" });

  useEffect(() => {
    if (model) {
      setDraft({ displayName: model.displayName, family: model.family });
    }
  }, [model]);

  if (!model) {
    return (
      <MissingRecordView
        title="Catalog Model not found"
        body="This Catalog Model may have been deleted or the link is incomplete."
        to={adminPaths.models()}
        label="Back to model catalog"
      />
    );
  }

  const selectedModel = model;
  const linkedRoutes = routes.filter((route) => route.publicModelId === model.id);
  const changed = draft.displayName !== model.displayName || draft.family !== model.family;

  async function handleSave(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (changed) {
      await onUpdate(selectedModel, draft);
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
        <div className="flex min-w-0 flex-col gap-2">
          <Button asChild type="button" variant="ghost" size="sm" className="w-fit">
            <Link to={adminPaths.models()}>Model Catalog</Link>
          </Button>
          <div>
            <div className="text-sm text-muted-foreground">Catalog Model</div>
            <h1 className="truncate font-mono text-2xl font-semibold">{model.id}</h1>
          </div>
        </div>
        <Button type="button" variant="outline" onClick={() => void onToggle(model)}>
          {model.enabled ? "Disable" : "Enable"}
        </Button>
      </div>

      <div className="grid gap-5 xl:grid-cols-[0.75fr_1.25fr]">
        <Card>
          <CardHeader>
            <CardTitle>Catalog details</CardTitle>
            <CardDescription>Public model identity and client-facing metadata.</CardDescription>
          </CardHeader>
          <CardContent>
            <form className="flex flex-col gap-4" onSubmit={handleSave}>
              <Field label="Display name" htmlFor="catalog-model-display-name">
                <Input
                  id="catalog-model-display-name"
                  value={draft.displayName}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, displayName: event.target.value }))
                  }
                />
              </Field>
              <Field label="Family" htmlFor="catalog-model-family">
                <Input
                  id="catalog-model-family"
                  value={draft.family}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, family: event.target.value }))
                  }
                />
              </Field>
              <div className="flex items-center justify-between gap-3 text-sm">
                <span className="text-muted-foreground">Status</span>
                {statusBadge(model.enabled ? "healthy" : "paused", model.enabled)}
              </div>
              <Button type="submit" disabled={!changed}>
                Save changes
              </Button>
            </form>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Provider Model Routes</CardTitle>
            <CardDescription>Bindings that make this Catalog Model reachable.</CardDescription>
          </CardHeader>
          <CardContent>
            {linkedRoutes.length === 0 ? (
              <EmptyNotice
                title="No provider routes"
                body="Add a route from the model catalog to make this Catalog Model routable."
              />
            ) : (
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Upstream Model</TableHead>
                      <TableHead>Provider Account</TableHead>
                      <TableHead>Role</TableHead>
                      <TableHead>Status</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {linkedRoutes.map((route) => (
                      <TableRow key={route.id}>
                        <TableCell>
                          <Link
                            to={adminPaths.route(route.id)}
                            className="font-mono text-xs underline-offset-4 hover:underline"
                          >
                            {route.upstreamModelId}
                          </Link>
                        </TableCell>
                        <TableCell>
                          <Link
                            to={adminPaths.account(route.providerAccountId)}
                            className="underline-offset-4 hover:underline"
                          >
                            {accountName(accounts, route.providerAccountId)}
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

export function ProviderRouteDetailView({
  accounts,
  models,
  routes,
  onToggle,
  onDelete,
}: {
  accounts: readonly ProviderAccount[];
  models: readonly ModelCatalogEntry[];
  routes: readonly ProviderModelRoute[];
  onToggle: (route: ProviderModelRoute) => Promise<void>;
  onDelete: (route: ProviderModelRoute) => Promise<boolean>;
}) {
  const { routeId } = useParams();
  const navigate = useNavigate();
  const route = routes.find((item) => item.id === routeId);

  if (!route) {
    return (
      <MissingRecordView
        title="Provider Model Route not found"
        body="This Provider Model Route may have been deleted or the link is incomplete."
        to={adminPaths.models()}
        label="Back to model catalog"
      />
    );
  }

  const selectedRoute = route;
  const model = models.find((item) => item.id === route.publicModelId);
  const account = accounts.find((item) => item.id === route.providerAccountId);

  async function handleDelete() {
    if (await onDelete(selectedRoute)) {
      void navigate(adminPaths.models());
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
        <div className="flex min-w-0 flex-col gap-2">
          <Button asChild type="button" variant="ghost" size="sm" className="w-fit">
            <Link to={adminPaths.models()}>Model Catalog</Link>
          </Button>
          <div>
            <div className="text-sm text-muted-foreground">Provider Model Route</div>
            <h1 className="truncate text-2xl font-semibold">
              {route.publicModelId} → {route.upstreamModelId}
            </h1>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="outline" onClick={() => void onToggle(route)}>
            {route.enabled ? "Disable" : "Enable"}
          </Button>
          <Button type="button" variant="destructive" onClick={handleDelete}>
            <Trash2Icon data-icon="inline-start" />
            Delete
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Binding details</CardTitle>
          <CardDescription>How this Catalog Model reaches its upstream model.</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className="grid gap-5 text-sm sm:grid-cols-2 xl:grid-cols-3">
            <div>
              <dt className="text-xs text-muted-foreground">Catalog Model</dt>
              <dd className="mt-1">
                <Link
                  to={adminPaths.model(route.publicModelId)}
                  className="font-mono text-xs underline-offset-4 hover:underline"
                >
                  {model?.id ?? route.publicModelId}
                </Link>
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Provider Account</dt>
              <dd className="mt-1">
                <Link
                  to={adminPaths.account(route.providerAccountId)}
                  className="underline-offset-4 hover:underline"
                >
                  {account?.name ?? route.providerAccountId}
                </Link>
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Upstream Model</dt>
              <dd className="mt-1 font-mono text-xs">{route.upstreamModelId}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Protocol</dt>
              <dd className="mt-1">{wireApiLabel(route.wireApi)}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Role</dt>
              <dd className="mt-1">{routeRoleBadge(route.role)}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Status</dt>
              <dd className="mt-1">{statusBadge(route.status, route.enabled)}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Policy</dt>
              <dd className="mt-1 text-xs text-muted-foreground">{formatRoutePolicy(route)}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Last used</dt>
              <dd className="mt-1">{formatDate(route.lastUsedAt)}</dd>
            </div>
            {route.lastError ? (
              <div className="sm:col-span-2 xl:col-span-3">
                <dt className="text-xs text-muted-foreground">Last error</dt>
                <dd className="mt-1 break-words text-destructive">{route.lastError}</dd>
              </div>
            ) : null}
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}

export function CreateModelSheet({
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

export function CreateRouteSheet({
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
