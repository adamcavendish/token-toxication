import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createMemoryRouter, matchRoutes, MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vite-plus/test";

import { ApiKeyDetailView, ApiKeysView } from "./admin-ui/api-keys";
import type { ApiKey } from "./types";
import {
  AdminRoutes,
  adminPathId,
  adminPathLabel,
  adminPaths,
  adminRouteDefinitions,
} from "./admin-routes";

const apiKey: ApiKey = {
  id: "key/west asia",
  name: "West Asia",
  description: "regional client credential",
  permissions: [],
  rateLimitPerMinute: 20,
  concurrencyLimit: 2,
  dailyCostLimit: 0,
  isActive: true,
  keyPreview: "tokentoxication-…west",
  createdAt: "2026-07-23T00:00:00Z",
  lastUsedAt: null,
  expiresAt: null,
};

describe("admin routes", () => {
  it("matches an encoded provider account link to its detail route", () => {
    const path = adminPaths.account("account/west asia");
    const match = matchRoutes(adminRouteDefinitions, path)?.at(-1);

    expect(path).toBe("/accounts/account%2Fwest%20asia");
    expect(match?.route.id).toBe("account-detail");
    expect(match?.params.accountId).toBe("account/west asia");
  });

  it("generates encoded API key links from durable record identifiers", () => {
    const markup = renderToStaticMarkup(
      createElement(
        MemoryRouter,
        { initialEntries: [adminPaths.keys()] },
        createElement(ApiKeysView, {
          apiKeys: [apiKey],
          onCreate: () => undefined,
          onToggle: () => undefined,
        }),
      ),
    );

    expect(markup).toContain('href="/keys/key%2Fwest%20asia"');
  });

  it("labels record and missing locations through the route module", () => {
    expect(adminPathLabel("/accounts/account%2Fwest%20asia")).toBe("Provider Account");
    expect(adminPathLabel("/keys/key%2Fwest%20asia")).toBe("API Key");
    expect(adminPathLabel("/does-not-exist")).toBe("Not found");
  });

  it("renders a collection back link when a durable record is missing", () => {
    const markup = renderToStaticMarkup(
      createElement(
        MemoryRouter,
        { initialEntries: [adminPaths.key("deleted-key")] },
        createElement(
          Routes,
          null,
          createElement(Route, {
            path: "/keys/:keyId",
            element: createElement(ApiKeyDetailView, {
              apiKeys: [],
              onToggle: async () => undefined,
              onDelete: async () => false,
            }),
          }),
        ),
      ),
    );

    expect(markup).toContain("API key not found");
    expect(markup).toContain('href="/keys"');
  });

  it("renders the route module's not-found entry for unknown locations", () => {
    const elements = Object.fromEntries(
      adminRouteDefinitions.map((route) => [route.id, createElement("span", null, route.id)]),
    ) as unknown as Parameters<typeof AdminRoutes>[0]["elements"];
    const markup = renderToStaticMarkup(
      createElement(
        MemoryRouter,
        { initialEntries: ["/does-not-exist"] },
        createElement(AdminRoutes, { elements }),
      ),
    );

    expect(markup).toContain("not-found");
  });

  it("keeps provider route details under the model catalog navigation", () => {
    expect(adminPathId("/routes/route-42")).toBe("route-detail");
  });

  it("restores the prior detail route when history moves back", async () => {
    const router = createMemoryRouter(adminRouteDefinitions, {
      initialEntries: [adminPaths.account("account-west"), adminPaths.route("route-42")],
      initialIndex: 1,
    });

    expect(router.state.matches.at(-1)?.route.id).toBe("route-detail");

    await router.navigate(-1);

    expect(router.state.matches.at(-1)?.route.id).toBe("account-detail");
    router.dispose();
  });
});
