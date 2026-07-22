import { Link, useParams } from "react-router";

import { adminPaths } from "../admin-routes";
import {
  formatCacheHitRate,
  formatDate,
  formatLogModel,
  formatNumber,
  formatRequestSummary,
  statusCodeBadge,
} from "./helpers";
import { EmptyNotice, MissingRecordView } from "./shared";
import type { RequestLog } from "../types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export function RequestLogDetailView({ logs }: { logs: readonly RequestLog[] }) {
  const { logId } = useParams();
  const log = logs.find((item) => item.id === logId);

  if (!log) {
    return (
      <MissingRecordView
        title="Request Log not found"
        body="This Request Log may have aged out of the current log collection or the link is incomplete."
        to={adminPaths.logs()}
        label="Back to request log"
      />
    );
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
        <div className="flex min-w-0 flex-col gap-2">
          <Button asChild type="button" variant="ghost" size="sm" className="w-fit">
            <Link to={adminPaths.logs()}>Request Log</Link>
          </Button>
          <div>
            <div className="text-sm text-muted-foreground">Request Log</div>
            <h1 className="truncate text-2xl font-semibold">{formatLogModel(log)}</h1>
            <p className="mt-1 text-sm text-muted-foreground">{formatDate(log.createdAt)}</p>
          </div>
        </div>
        {statusCodeBadge(log.statusCode)}
      </div>

      <div className="grid gap-5 xl:grid-cols-[0.75fr_1.25fr]">
        <Card>
          <CardHeader>
            <CardTitle>Request summary</CardTitle>
            <CardDescription>
              Relay result and accounting for this completed attempt.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <dl className="grid gap-4 text-sm">
              <div>
                <dt className="text-xs text-muted-foreground">Latency</dt>
                <dd className="mt-1">{log.latencyMs}ms</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">Tokens</dt>
                <dd className="mt-1">
                  {formatNumber(log.inputTokens + log.outputTokens)} total ·{" "}
                  {formatCacheHitRate(log.cachedInputTokens, log.inputTokens)} cache hit
                </dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">Client key</dt>
                <dd className="mt-1 font-mono text-xs">{log.apiKeyId}</dd>
              </div>
              {log.providerAccountId ? (
                <div>
                  <dt className="text-xs text-muted-foreground">Provider Account</dt>
                  <dd className="mt-1">
                    <Link
                      to={adminPaths.account(log.providerAccountId)}
                      className="font-mono text-xs underline-offset-4 hover:underline"
                    >
                      {log.providerAccountId}
                    </Link>
                  </dd>
                </div>
              ) : null}
            </dl>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Routing details</CardTitle>
            <CardDescription>{formatRequestSummary(log)}</CardDescription>
          </CardHeader>
          <CardContent>
            <RequestLogFacts log={log} />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

export function RequestLogsView({
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
        <div className="hidden xl:block">
          <Table className="table-fixed">
            <colgroup>
              <col className="w-32" />
              <col className="w-[18%]" />
              <col className="w-[14%]" />
              <col />
              <col className="w-20" />
              <col className="w-24" />
              <col className="w-24" />
              <col className="w-24" />
            </colgroup>
            <TableHeader>
              <TableRow>
                <TableHead>Time</TableHead>
                <TableHead>Path</TableHead>
                <TableHead>Model</TableHead>
                <TableHead>Request</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Latency</TableHead>
                <TableHead className="text-right">Tokens</TableHead>
                <TableHead className="text-right">Cache hit</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {logs.map((log) => (
                <TableRow key={log.id}>
                  <TableCell className="text-xs text-muted-foreground">
                    <Link
                      to={adminPaths.log(log.id)}
                      className="underline-offset-4 hover:underline"
                    >
                      {formatDate(log.createdAt)}
                    </Link>
                  </TableCell>
                  <TableCell>
                    <div className="flex min-w-0 flex-col gap-1">
                      <Link
                        to={adminPaths.log(log.id)}
                        className="truncate font-mono text-xs underline-offset-4 hover:underline"
                        title={log.path}
                      >
                        {log.path}
                      </Link>
                      {log.upstreamUrl ? (
                        <span
                          className="truncate font-mono text-xs text-muted-foreground"
                          title={log.upstreamUrl}
                        >
                          {log.upstreamUrl}
                        </span>
                      ) : null}
                    </div>
                  </TableCell>
                  <TableCell>
                    <Link
                      to={adminPaths.log(log.id)}
                      className="block truncate underline-offset-4 hover:underline"
                      title={formatLogModel(log)}
                    >
                      {formatLogModel(log)}
                    </Link>
                  </TableCell>
                  <TableCell>
                    <span
                      className="block truncate text-xs text-muted-foreground"
                      title={formatRequestSummary(log)}
                    >
                      {formatRequestSummary(log)}
                    </span>
                  </TableCell>
                  <TableCell>{statusCodeBadge(log.statusCode)}</TableCell>
                  <TableCell className="text-right tabular-nums">{log.latencyMs}ms</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatNumber(log.inputTokens + log.outputTokens)}
                  </TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatCacheHitRate(log.cachedInputTokens, log.inputTokens)}
                  </TableCell>
                </TableRow>
              ))}
              {logs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={8}>
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
        <div className="hidden md:block xl:hidden">
          <Table className="table-fixed">
            <colgroup>
              <col />
              <col className="w-20" />
              <col className="w-24" />
              <col className="w-24" />
              <col className="w-24" />
            </colgroup>
            <TableHeader>
              <TableRow>
                <TableHead>Request</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Latency</TableHead>
                <TableHead className="text-right">Tokens</TableHead>
                <TableHead className="text-right">Cache hit</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {logs.map((log) => (
                <TableRow key={log.id}>
                  <TableCell className="whitespace-normal">
                    <div className="min-w-0">
                      <Link
                        to={adminPaths.log(log.id)}
                        className="block truncate font-medium underline-offset-4 hover:underline"
                        title={formatLogModel(log)}
                      >
                        {formatLogModel(log)}
                      </Link>
                      <div
                        className="mt-1 truncate text-xs text-muted-foreground"
                        title={formatRequestSummary(log)}
                      >
                        {formatRequestSummary(log)}
                      </div>
                      <RequestLogDetails log={log} />
                    </div>
                  </TableCell>
                  <TableCell>{statusCodeBadge(log.statusCode)}</TableCell>
                  <TableCell className="text-right tabular-nums">{log.latencyMs}ms</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatNumber(log.inputTokens + log.outputTokens)}
                  </TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatCacheHitRate(log.cachedInputTokens, log.inputTokens)}
                  </TableCell>
                </TableRow>
              ))}
              {logs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5}>
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
              <div key={log.id} className="flex flex-col gap-3 rounded-md border p-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <Link
                      to={adminPaths.log(log.id)}
                      className="block truncate font-mono text-xs underline-offset-4 hover:underline"
                      title={log.path}
                    >
                      {log.path}
                    </Link>
                    <div
                      className="truncate text-xs text-muted-foreground"
                      title={formatLogModel(log)}
                    >
                      {formatLogModel(log)}
                    </div>
                    {log.upstreamUrl ? (
                      <div
                        className="truncate font-mono text-xs text-muted-foreground"
                        title={log.upstreamUrl}
                      >
                        {log.upstreamUrl}
                      </div>
                    ) : null}
                  </div>
                  {statusCodeBadge(log.statusCode)}
                </div>
                <div className="grid grid-cols-3 gap-2 text-xs text-muted-foreground">
                  <span>{formatDate(log.createdAt)}</span>
                  <span className="text-right tabular-nums">{log.latencyMs}ms</span>
                  <span className="text-right tabular-nums">
                    <span className="block">
                      {formatNumber(log.inputTokens + log.outputTokens)} tokens
                    </span>
                    <span className="block">
                      {formatCacheHitRate(log.cachedInputTokens, log.inputTokens)} cache hit
                    </span>
                  </span>
                </div>
                <div className="break-words text-xs leading-5 text-muted-foreground">
                  {formatRequestSummary(log)}
                </div>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function RequestLogDetails({ log }: { log: RequestLog }) {
  return (
    <details className="group mt-2 text-xs text-muted-foreground">
      <summary className="w-fit cursor-pointer list-none text-foreground underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
        Routing details
      </summary>
      <RequestLogFacts log={log} />
    </details>
  );
}

function RequestLogFacts({ log }: { log: RequestLog }) {
  return (
    <dl className="mt-2 grid gap-1 border-l pl-3 text-xs text-muted-foreground">
      <div className="flex gap-2">
        <dt className="shrink-0">Time</dt>
        <dd>{formatDate(log.createdAt)}</dd>
      </div>
      <div className="flex min-w-0 gap-2">
        <dt className="shrink-0">Path</dt>
        <dd className="min-w-0 truncate font-mono" title={log.path}>
          {log.path}
        </dd>
      </div>
      {log.upstreamUrl ? (
        <div className="flex min-w-0 gap-2">
          <dt className="shrink-0">Upstream</dt>
          <dd className="min-w-0 truncate font-mono" title={log.upstreamUrl}>
            {log.upstreamUrl}
          </dd>
        </div>
      ) : null}
      <div className="flex gap-2">
        <dt className="shrink-0">Tokens</dt>
        <dd>
          {formatNumber(log.inputTokens)} input · {formatNumber(log.cachedInputTokens)} cached ·{" "}
          {formatNumber(log.outputTokens)} output
        </dd>
      </div>
      {log.error ? (
        <div className="flex min-w-0 gap-2 text-destructive">
          <dt className="shrink-0">Error</dt>
          <dd className="min-w-0 truncate" title={log.error}>
            {log.error}
          </dd>
        </div>
      ) : null}
    </dl>
  );
}
