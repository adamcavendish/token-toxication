import type React from "react";
import { Link } from "react-router";
import {
  ActivityIcon,
  CableIcon,
  GaugeIcon,
  KeyRoundIcon,
  PlusIcon,
  TerminalSquareIcon,
} from "lucide-react";

import { adminPaths } from "../admin-routes";
import {
  formatCacheHitRate,
  formatChartInterval,
  formatChartTime,
  formatNumber,
  formatRequestCount,
  niceChartMaximum,
  statusBadge,
  wireApiLabel,
} from "./helpers";
import { RequestLogsView } from "./logs";
import { EmptyNotice } from "./shared";
import type { Dashboard, RequestTrend } from "../types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export function Overview({
  dashboard,
  onCreateKey,
  onCreateAccount,
}: {
  dashboard: Dashboard;
  onCreateKey: () => void;
  onCreateAccount: () => void;
}) {
  return (
    <div className="flex flex-col gap-5">
      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
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
          title="Cache hit rate"
          value={formatCacheHitRate(
            dashboard.usage.cachedInputTokensToday,
            dashboard.usage.inputTokensToday,
          )}
          detail={`${formatNumber(dashboard.usage.cachedInputTokensToday)} / ${formatNumber(dashboard.usage.inputTokensToday)} input tokens`}
          icon={GaugeIcon}
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
              <CardDescription>
                Completed relay requests across clock-aligned intervals.
              </CardDescription>
            </div>
            <Button type="button" onClick={onCreateKey}>
              <PlusIcon data-icon="inline-start" />
              API Key
            </Button>
          </CardHeader>
          <CardContent>
            <TrendChart trend={dashboard.requestTrend} />
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
                    <Link
                      to={adminPaths.account(account.id)}
                      className="block truncate text-sm font-medium underline-offset-4 hover:underline"
                    >
                      {account.name}
                    </Link>
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

function TrendChart({ trend }: { trend: RequestTrend }) {
  const values = trend.buckets.map((bucket) => bucket.requestCount);
  const peak = Math.max(...values, 0);
  const max = niceChartMaximum(peak);
  const chartTop = 4;
  const chartBottom = 96;
  const pointPositions = values.map((value, index) => {
    const x = (index / Math.max(values.length - 1, 1)) * 100;
    const y = chartBottom - (value / max) * (chartBottom - chartTop);
    return { x, y, value, bucket: trend.buckets[index] };
  });
  const points = pointPositions.map(({ x, y }) => `${x},${y}`).join(" ");
  const total = values.reduce((sum, value) => sum + value, 0);
  const completedValues = values.slice(0, -1);
  const completedAverage =
    completedValues.length > 0
      ? completedValues.reduce((sum, value) => sum + value, 0) / completedValues.length
      : 0;
  const intervalMinutes = Math.round(trend.bucketDurationSeconds / 60);
  const firstBucket = trend.buckets[0];
  const middleBucket = trend.buckets[Math.floor((trend.buckets.length - 1) / 2)];

  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-lg border bg-muted/10 p-4">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <div className="text-sm font-medium">Requests per {intervalMinutes} minutes</div>
            <p className="text-xs text-muted-foreground">
              Each point counts completed requests in one interval. Hover or focus for details.
            </p>
          </div>
          <Badge variant="outline">
            {values.length} × {intervalMinutes} min
          </Badge>
        </div>

        <div className="mt-4 grid grid-cols-[2.5rem_minmax(0,1fr)] gap-x-2">
          <div className="relative h-40 text-right text-[11px] tabular-nums text-muted-foreground">
            <span className="absolute top-[4%] right-0 -translate-y-1/2">{formatNumber(max)}</span>
            <span className="absolute top-1/2 right-0 -translate-y-1/2">
              {formatNumber(max / 2)}
            </span>
            <span className="absolute right-0 bottom-[4%] translate-y-1/2">0</span>
          </div>

          <div className="relative h-40" aria-label="Request count by local time">
            <svg
              viewBox="0 0 100 100"
              preserveAspectRatio="none"
              className="pointer-events-none absolute inset-0 h-full w-full text-foreground/70"
              role="img"
              aria-label={`Request counts across ${values.length} clock-aligned ${intervalMinutes}-minute intervals`}
            >
              {[chartTop, 50, chartBottom].map((y) => (
                <line
                  key={y}
                  x1="0"
                  x2="100"
                  y1={y}
                  y2={y}
                  className="stroke-border"
                  strokeWidth="1"
                  strokeDasharray={y === chartBottom ? undefined : "3 3"}
                  vectorEffect="non-scaling-stroke"
                />
              ))}
              <polygon
                points={`0,${chartBottom} ${points} 100,${chartBottom}`}
                className="fill-primary/5"
              />
              <polyline
                points={points}
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
              />
            </svg>

            {pointPositions.map(({ x, y, value, bucket }) => (
              <Tooltip key={bucket.startedAt}>
                <TooltipTrigger asChild>
                  <button
                    type="button"
                    className="group absolute z-10 flex size-7 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    style={{ left: `${x}%`, top: `${y}%` }}
                    aria-label={`${formatChartInterval(bucket.startedAt, trend.bucketDurationSeconds)}: ${formatRequestCount(value)}`}
                  >
                    <span className="size-2.5 rounded-full border-2 border-background bg-foreground shadow-sm transition-transform group-hover:scale-125 group-focus-visible:scale-125" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" sideOffset={4}>
                  <span className="font-medium">
                    {formatChartInterval(bucket.startedAt, trend.bucketDurationSeconds)}
                  </span>
                  <span>{formatRequestCount(value)}</span>
                </TooltipContent>
              </Tooltip>
            ))}

            {total === 0 ? (
              <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
                <span className="rounded-md border bg-background/90 px-2 py-1 text-xs text-muted-foreground shadow-sm">
                  No requests in this window
                </span>
              </div>
            ) : null}
          </div>

          <div />
          <div className="mt-2 flex justify-between text-[11px] tabular-nums text-muted-foreground">
            <span>{firstBucket ? formatChartTime(firstBucket.startedAt) : ""}</span>
            <span>{middleBucket ? formatChartTime(middleBucket.startedAt) : ""}</span>
            <span>Now</span>
          </div>
          <div />
          <div className="mt-1 text-center text-[11px] text-muted-foreground">Local time</div>
        </div>

        <p className="mt-3 text-xs text-muted-foreground">
          Intervals align to local {intervalMinutes}-minute boundaries. The rightmost interval is
          still in progress, so its count is excluded from the average.
        </p>
      </div>
      <div className="grid gap-3 md:grid-cols-3">
        <ChartStat label="Window total" value={formatNumber(total)} />
        <ChartStat label={`Peak / ${intervalMinutes} min`} value={formatNumber(peak)} />
        <ChartStat
          label={`Completed avg / ${intervalMinutes} min`}
          value={completedAverage.toLocaleString(undefined, { maximumFractionDigits: 1 })}
        />
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
