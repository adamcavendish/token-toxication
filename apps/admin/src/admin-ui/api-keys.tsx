import type React from "react";
import { Link, useNavigate, useParams } from "react-router";
import { KeyRoundIcon, PlusIcon, Trash2Icon } from "lucide-react";

import { adminPaths } from "../admin-routes";
import { formatDate } from "./helpers";
import { EmptyNotice, Field, MissingRecordView } from "./shared";
import type { CreateKeyForm } from "./types";
import type { ApiKey } from "../types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";

export function ApiKeysView({
  apiKeys,
  onCreate,
  onToggle,
}: {
  apiKeys: ApiKey[];
  onCreate: () => void;
  onToggle: (key: ApiKey) => void;
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
                      <Link
                        to={adminPaths.key(key.id)}
                        className="font-medium underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      >
                        {key.name}
                      </Link>
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
                    <Button type="button" variant="outline" size="sm" onClick={() => onToggle(key)}>
                      {key.isActive ? "Active" : "Paused"}
                    </Button>
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
                    <Link
                      to={adminPaths.key(key.id)}
                      className="block truncate text-sm font-medium underline-offset-4 hover:underline"
                    >
                      {key.name}
                    </Link>
                    <div className="truncate font-mono text-xs text-muted-foreground">
                      {key.keyPreview}
                    </div>
                  </div>
                  <Button type="button" variant="outline" size="sm" onClick={() => onToggle(key)}>
                    {key.isActive ? "Active" : "Paused"}
                  </Button>
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

export function ApiKeyDetailView({
  apiKeys,
  onToggle,
  onDelete,
}: {
  apiKeys: readonly ApiKey[];
  onToggle: (key: ApiKey) => Promise<void>;
  onDelete: (key: ApiKey) => Promise<boolean>;
}) {
  const { keyId } = useParams();
  const navigate = useNavigate();
  const key = apiKeys.find((item) => item.id === keyId);

  if (!key) {
    return (
      <MissingRecordView
        title="API key not found"
        body="This API key may have been deleted or the link is incomplete."
        to={adminPaths.keys()}
        label="Back to API keys"
      />
    );
  }

  const selectedKey = key;

  async function handleDelete() {
    if (await onDelete(selectedKey)) {
      void navigate(adminPaths.keys());
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
        <div className="flex min-w-0 flex-col gap-2">
          <Button asChild type="button" variant="ghost" size="sm" className="w-fit">
            <Link to={adminPaths.keys()}>API Keys</Link>
          </Button>
          <div>
            <div className="text-sm text-muted-foreground">API Key</div>
            <h1 className="truncate text-2xl font-semibold">{selectedKey.name}</h1>
            <p className="mt-1 font-mono text-sm text-muted-foreground">{selectedKey.keyPreview}</p>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="outline" onClick={() => void onToggle(key)}>
            {selectedKey.isActive ? "Disable" : "Enable"}
          </Button>
          <Button type="button" variant="destructive" onClick={() => void handleDelete()}>
            <Trash2Icon data-icon="inline-start" />
            Delete
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Credential details</CardTitle>
          <CardDescription>
            Stored metadata and request limits for this client credential.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <dl className="grid gap-5 text-sm sm:grid-cols-2 xl:grid-cols-3">
            <div>
              <dt className="text-xs text-muted-foreground">Description</dt>
              <dd className="mt-1">{selectedKey.description || "No description"}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Permissions</dt>
              <dd className="mt-1">
                {selectedKey.permissions.length === 0
                  ? "All services"
                  : selectedKey.permissions.join(", ")}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Status</dt>
              <dd className="mt-1">{selectedKey.isActive ? "Active" : "Paused"}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Rate limit</dt>
              <dd className="mt-1">{selectedKey.rateLimitPerMinute || "No"} rpm</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Concurrency limit</dt>
              <dd className="mt-1">{selectedKey.concurrencyLimit || "No"} concurrent requests</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Daily cost limit</dt>
              <dd className="mt-1">
                {selectedKey.dailyCostLimit ? `$${selectedKey.dailyCostLimit}` : "No limit"}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Created</dt>
              <dd className="mt-1">{formatDate(selectedKey.createdAt)}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Last used</dt>
              <dd className="mt-1">{formatDate(selectedKey.lastUsedAt)}</dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Expires</dt>
              <dd className="mt-1">{formatDate(selectedKey.expiresAt)}</dd>
            </div>
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}

export function CreateKeySheet({
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
