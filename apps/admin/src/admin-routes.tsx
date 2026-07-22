import type { ReactNode } from "react";
import { generatePath, matchRoutes, Route, Routes, type RouteObject } from "react-router";

const adminRouteMetadata = [
  { id: "overview", path: "/", label: "Overview" },
  { id: "keys", path: "/keys", label: "API Keys" },
  { id: "key-detail", path: "/keys/:keyId", label: "API Key" },
  { id: "accounts", path: "/accounts", label: "Provider Accounts" },
  { id: "account-detail", path: "/accounts/:accountId", label: "Provider Account" },
  { id: "models", path: "/models", label: "Model Catalog" },
  { id: "model-detail", path: "/models/:modelId", label: "Catalog Model" },
  { id: "route-detail", path: "/routes/:routeId", label: "Provider Model Route" },
  { id: "setup", path: "/setup", label: "Client Setup" },
  { id: "logs", path: "/logs", label: "Request Log" },
  { id: "log-detail", path: "/logs/:logId", label: "Request Log" },
  { id: "settings", path: "/settings", label: "Settings" },
  { id: "not-found", path: "*", label: "Not found" },
] as const;

export type AdminRouteId = (typeof adminRouteMetadata)[number]["id"];

type AdminRouteMetadata = (typeof adminRouteMetadata)[number];

const adminRouteById = Object.fromEntries(
  adminRouteMetadata.map((route) => [route.id, route]),
) as Record<AdminRouteId, AdminRouteMetadata>;

export const adminRouteDefinitions: Array<RouteObject & { id: AdminRouteId }> =
  adminRouteMetadata.map(({ id, path }) => ({ id, path }));

export const adminNavigation: ReadonlyArray<{
  id: AdminRouteId;
  activeIds: readonly AdminRouteId[];
}> = [
  { id: "overview", activeIds: ["overview"] },
  { id: "keys", activeIds: ["keys", "key-detail"] },
  { id: "accounts", activeIds: ["accounts", "account-detail"] },
  { id: "models", activeIds: ["models", "model-detail", "route-detail"] },
  { id: "setup", activeIds: ["setup"] },
  { id: "logs", activeIds: ["logs", "log-detail"] },
  { id: "settings", activeIds: ["settings"] },
];

export function adminRoutePath(id: AdminRouteId) {
  return adminRouteById[id].path;
}

export const adminPaths = {
  overview: () => adminRoutePath("overview"),
  keys: () => adminRoutePath("keys"),
  key: (keyId: string) => generatePath(adminRoutePath("key-detail"), { keyId }),
  accounts: () => adminRoutePath("accounts"),
  account: (accountId: string) => generatePath(adminRoutePath("account-detail"), { accountId }),
  models: () => adminRoutePath("models"),
  model: (modelId: string) => generatePath(adminRoutePath("model-detail"), { modelId }),
  route: (routeId: string) => generatePath(adminRoutePath("route-detail"), { routeId }),
  setup: () => adminRoutePath("setup"),
  logs: () => adminRoutePath("logs"),
  log: (logId: string) => generatePath(adminRoutePath("log-detail"), { logId }),
  settings: () => adminRoutePath("settings"),
};

export function adminRouteLabel(id: AdminRouteId): string {
  return adminRouteById[id].label;
}

export function adminPathLabel(pathname: string): string {
  return adminRouteLabel(adminPathId(pathname));
}

export function adminPathId(pathname: string): AdminRouteId {
  const routeId = matchRoutes(adminRouteDefinitions, pathname)?.at(-1)?.route.id;
  return routeId && routeId in adminRouteById ? (routeId as AdminRouteId) : "not-found";
}

export function AdminRoutes({ elements }: { elements: Record<AdminRouteId, ReactNode> }) {
  return (
    <Routes>
      {adminRouteDefinitions.map((route) => (
        <Route key={route.id} id={route.id} path={route.path} element={elements[route.id]} />
      ))}
    </Routes>
  );
}
