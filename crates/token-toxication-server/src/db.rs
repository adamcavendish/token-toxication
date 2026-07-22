use std::{path::Path, sync::Arc};

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    auth::{hash_secret, key_preview},
    codex_subscription::{canonicalize_legacy_codex_base_url, is_codex_subscription_auth},
    models::{
        ApiKeyRecord, ApiKeyView, CreateApiKeyRequest, CreateModelCatalogEntryRequest,
        CreateProviderAccountRequest, CreateProviderModelRouteRequest, Dashboard,
        ModelCatalogEntry, ProviderAccount, ProviderAccountRecord, ProviderModelRoute, RequestLog,
        RequestSummary, RequestTrend, RequestTrendBucket, UpdateApiKeyRequest,
        UpdateModelCatalogEntryRequest, UpdateProviderAccountRequest,
        UpdateProviderModelRouteRequest, UsageSummary,
    },
    provider_catalog::{default_wire_api_for_provider, normalize_provider_alias},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutableModelCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub provider: String,
    pub wire_api: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ProviderRouteSelection {
    pub account: ProviderAccountRecord,
    pub route_id: String,
    pub public_model_id: String,
    pub upstream_model_id: String,
    pub strip_params: Vec<String>,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub async fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        }

        let mut conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                key_hash TEXT NOT NULL UNIQUE,
                key_preview TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                permissions TEXT NOT NULL DEFAULT '[]',
                rate_limit_per_minute INTEGER NOT NULL DEFAULT 0,
                concurrency_limit INTEGER NOT NULL DEFAULT 0,
                daily_cost_limit REAL NOT NULL DEFAULT 0,
                expires_at TEXT,
                created_at TEXT NOT NULL,
                last_used_at TEXT
            );

            CREATE TABLE IF NOT EXISTS provider_accounts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider TEXT NOT NULL,
                base_url TEXT NOT NULL,
                auth_mode TEXT NOT NULL,
                wire_api TEXT NOT NULL DEFAULT 'anthropic-messages',
                api_key TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'healthy',
                last_error TEXT,
                created_at TEXT NOT NULL,
                last_used_at TEXT
            );

            CREATE TABLE IF NOT EXISTS request_logs (
                id TEXT PRIMARY KEY,
                api_key_id TEXT NOT NULL,
                provider_account_id TEXT,
                method TEXT NOT NULL,
                path TEXT NOT NULL,
                model TEXT,
                upstream_model TEXT,
                upstream_url TEXT,
                request_summary TEXT,
                status_code INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                cached_input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                error TEXT
            );

            CREATE TABLE IF NOT EXISTS admin_sessions (
                token_hash TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS model_catalog (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                family TEXT NOT NULL DEFAULT 'other',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS provider_model_routes (
                id TEXT PRIMARY KEY,
                public_model_id TEXT NOT NULL,
                provider_account_id TEXT NOT NULL,
                upstream_model_id TEXT NOT NULL,
                wire_api TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'primary',
                enabled INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'healthy',
                last_error TEXT,
                last_status_code INTEGER,
                cooldown_until TEXT,
                last_used_at TEXT,
                strip_params TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                FOREIGN KEY(public_model_id) REFERENCES model_catalog(id) ON DELETE CASCADE,
                FOREIGN KEY(provider_account_id) REFERENCES provider_accounts(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_request_logs_created_at ON request_logs(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);
            "#,
        )?;
        ensure_column(
            &conn,
            "provider_accounts",
            "wire_api",
            "TEXT NOT NULL DEFAULT 'anthropic-messages'",
        )?;
        ensure_column(&conn, "request_logs", "upstream_model", "TEXT")?;
        ensure_column(&conn, "request_logs", "upstream_url", "TEXT")?;
        ensure_column(&conn, "request_logs", "request_summary", "TEXT")?;
        ensure_column(
            &conn,
            "request_logs",
            "cached_input_tokens",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "provider_model_routes",
            "status",
            "TEXT NOT NULL DEFAULT 'healthy'",
        )?;
        ensure_column(&conn, "provider_model_routes", "last_error", "TEXT")?;
        ensure_column(
            &conn,
            "provider_model_routes",
            "last_status_code",
            "INTEGER",
        )?;
        ensure_column(&conn, "provider_model_routes", "cooldown_until", "TEXT")?;
        ensure_column(&conn, "provider_model_routes", "last_used_at", "TEXT")?;
        ensure_column(
            &conn,
            "provider_model_routes",
            "strip_params",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        let migrated_codex_accounts = migrate_legacy_codex_base_urls(&mut conn)?;
        if migrated_codex_accounts > 0 {
            tracing::info!(
                migrated_codex_accounts,
                "migrated legacy Codex account base URLs"
            );
        }
        conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_provider_accounts_wire_api
                ON provider_accounts(wire_api, is_active, status);
            CREATE INDEX IF NOT EXISTS idx_model_catalog_enabled
                ON model_catalog(enabled, id);
            CREATE INDEX IF NOT EXISTS idx_provider_model_routes_lookup
                ON provider_model_routes(public_model_id, wire_api, enabled, role);
            CREATE INDEX IF NOT EXISTS idx_provider_model_routes_health
                ON provider_model_routes(enabled, status, cooldown_until);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_model_routes_primary
                ON provider_model_routes(public_model_id, wire_api)
                WHERE enabled = 1 AND role = 'primary';
            "#,
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn create_admin_session(
        &self,
        token: &str,
        username: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO admin_sessions (token_hash, username, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![hash_secret(token), username, expires_at.to_rfc3339(), Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub async fn validate_admin_session(
        &self,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let record = conn
            .query_row(
                "SELECT username, expires_at FROM admin_sessions WHERE token_hash = ?1",
                params![hash_secret(token)],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;

        let Some((username, expires_at)) = record else {
            return Ok(None);
        };
        let expires_at = parse_time_opt(Some(expires_at.as_str())).unwrap_or(now);
        if expires_at <= now {
            conn.execute(
                "DELETE FROM admin_sessions WHERE token_hash = ?1",
                params![hash_secret(token)],
            )?;
            return Ok(None);
        }
        Ok(Some(username))
    }

    pub async fn delete_admin_session(&self, token: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM admin_sessions WHERE token_hash = ?1",
            params![hash_secret(token)],
        )?;
        Ok(())
    }

    pub async fn create_api_key(
        &self,
        input: CreateApiKeyRequest,
        secret: &str,
    ) -> Result<ApiKeyView, rusqlite::Error> {
        let now = Utc::now();
        let key = ApiKeyView {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            description: input.description,
            key_preview: key_preview(secret),
            is_active: true,
            permissions: input.permissions,
            rate_limit_per_minute: input.rate_limit_per_minute,
            concurrency_limit: input.concurrency_limit,
            daily_cost_limit: input.daily_cost_limit,
            expires_at: input.expires_at,
            created_at: now,
            last_used_at: None,
        };

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO api_keys
             (id, name, description, key_hash, key_preview, is_active, permissions,
              rate_limit_per_minute, concurrency_limit, daily_cost_limit, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &key.id,
                &key.name,
                &key.description,
                hash_secret(secret),
                &key.key_preview,
                bool_to_i64(key.is_active),
                serde_json::to_string(&key.permissions).unwrap_or_else(|_| "[]".into()),
                key.rate_limit_per_minute,
                key.concurrency_limit,
                key.daily_cost_limit,
                key.expires_at.map(|value| value.to_rfc3339()),
                key.created_at.to_rfc3339(),
            ],
        )?;

        Ok(key)
    }

    pub async fn list_api_keys(&self) -> Result<Vec<ApiKeyView>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, key_hash, key_preview, is_active, permissions,
                    rate_limit_per_minute, concurrency_limit, daily_cost_limit, expires_at,
                    created_at, last_used_at
             FROM api_keys
             ORDER BY created_at DESC",
        )?;
        rows_to_api_keys(&mut stmt, params![])
    }

    pub async fn validate_api_key(
        &self,
        secret: &str,
    ) -> Result<Option<ApiKeyRecord>, rusqlite::Error> {
        let hash = hash_secret(secret);
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, key_hash, key_preview, is_active, permissions,
                    rate_limit_per_minute, concurrency_limit, daily_cost_limit, expires_at,
                    created_at, last_used_at
             FROM api_keys WHERE key_hash = ?1",
        )?;
        let record = stmt.query_row(params![hash], api_key_from_row).optional()?;
        let Some(record) = record else {
            return Ok(None);
        };

        if !record.view.is_active {
            return Ok(None);
        }
        if record
            .view
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
        {
            return Ok(None);
        }

        conn.execute(
            "UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), record.view.id],
        )?;
        Ok(Some(record))
    }

    pub async fn update_api_key(
        &self,
        id: &str,
        input: UpdateApiKeyRequest,
    ) -> Result<ApiKeyView, rusqlite::Error> {
        let current = self
            .get_api_key(id)
            .await?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let updated = ApiKeyView {
            id: current.id,
            name: input.name.unwrap_or(current.name),
            description: input.description.unwrap_or(current.description),
            key_preview: current.key_preview,
            is_active: input.is_active.unwrap_or(current.is_active),
            permissions: input.permissions.unwrap_or(current.permissions),
            rate_limit_per_minute: input
                .rate_limit_per_minute
                .unwrap_or(current.rate_limit_per_minute),
            concurrency_limit: input.concurrency_limit.unwrap_or(current.concurrency_limit),
            daily_cost_limit: input.daily_cost_limit.unwrap_or(current.daily_cost_limit),
            expires_at: input.expires_at.unwrap_or(current.expires_at),
            created_at: current.created_at,
            last_used_at: current.last_used_at,
        };
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE api_keys SET name = ?1, description = ?2, is_active = ?3, permissions = ?4,
             rate_limit_per_minute = ?5, concurrency_limit = ?6, daily_cost_limit = ?7, expires_at = ?8
             WHERE id = ?9",
            params![
                &updated.name,
                &updated.description,
                bool_to_i64(updated.is_active),
                serde_json::to_string(&updated.permissions).unwrap_or_else(|_| "[]".into()),
                updated.rate_limit_per_minute,
                updated.concurrency_limit,
                updated.daily_cost_limit,
                updated.expires_at.map(|value| value.to_rfc3339()),
                &updated.id,
            ],
        )?;
        Ok(updated)
    }

    pub async fn delete_api_key(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let deleted = conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }

    pub async fn get_api_key(&self, id: &str) -> Result<Option<ApiKeyView>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, key_hash, key_preview, is_active, permissions,
                    rate_limit_per_minute, concurrency_limit, daily_cost_limit, expires_at,
                    created_at, last_used_at
             FROM api_keys WHERE id = ?1",
        )?;
        stmt.query_row(params![id], api_key_from_row)
            .optional()
            .map(|record| record.map(|record| record.view))
    }

    pub async fn create_provider_account(
        &self,
        input: CreateProviderAccountRequest,
    ) -> Result<ProviderAccount, rusqlite::Error> {
        let now = Utc::now();
        let provider = normalize_provider(&input.provider);
        let auth_mode = normalize_auth_mode(&input.auth_mode);
        let account = ProviderAccount {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            provider,
            base_url: normalize_provider_base_url(&input.base_url, &auth_mode),
            auth_mode,
            wire_api: normalize_wire_api(&input.wire_api, &input.provider),
            is_active: input.is_active,
            priority: input.priority,
            status: "healthy".to_string(),
            last_error: None,
            created_at: now,
            last_used_at: None,
        };

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO provider_accounts
             (id, name, provider, base_url, auth_mode, wire_api, api_key, is_active,
              priority, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &account.id,
                &account.name,
                &account.provider,
                &account.base_url,
                &account.auth_mode,
                &account.wire_api,
                input.api_key,
                bool_to_i64(account.is_active),
                account.priority,
                &account.status,
                account.created_at.to_rfc3339(),
            ],
        )?;
        Ok(account)
    }

    pub async fn list_provider_accounts(&self) -> Result<Vec<ProviderAccount>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, base_url, auth_mode, wire_api, api_key, is_active,
                    priority, status, last_error, created_at, last_used_at
             FROM provider_accounts
             ORDER BY priority DESC, created_at DESC",
        )?;
        rows_to_accounts(&mut stmt, params![])
    }

    pub async fn list_model_catalog(&self) -> Result<Vec<ModelCatalogEntry>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, display_name, family, enabled, created_at
             FROM model_catalog
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], model_catalog_from_row)?;
        rows.collect()
    }

    pub async fn create_model_catalog_entry(
        &self,
        input: CreateModelCatalogEntryRequest,
    ) -> Result<ModelCatalogEntry, rusqlite::Error> {
        let now = Utc::now();
        let id = input.id.trim().to_string();
        let display_name = default_display_name(input.display_name, &id);
        let entry = ModelCatalogEntry {
            id,
            display_name,
            family: normalize_family(&input.family),
            enabled: input.enabled,
            created_at: now,
        };
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO model_catalog (id, display_name, family, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &entry.id,
                &entry.display_name,
                &entry.family,
                bool_to_i64(entry.enabled),
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(entry)
    }

    pub async fn update_model_catalog_entry(
        &self,
        id: &str,
        input: UpdateModelCatalogEntryRequest,
    ) -> Result<ModelCatalogEntry, rusqlite::Error> {
        let current = self
            .get_model_catalog_entry(id)
            .await?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let updated = ModelCatalogEntry {
            id: current.id,
            display_name: input
                .display_name
                .map(|value| default_display_name(value, &current.display_name))
                .unwrap_or(current.display_name),
            family: input
                .family
                .map(|value| normalize_family(&value))
                .unwrap_or(current.family),
            enabled: input.enabled.unwrap_or(current.enabled),
            created_at: current.created_at,
        };
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE model_catalog
             SET display_name = ?1, family = ?2, enabled = ?3
             WHERE id = ?4",
            params![
                &updated.display_name,
                &updated.family,
                bool_to_i64(updated.enabled),
                &updated.id,
            ],
        )?;
        Ok(updated)
    }

    pub async fn get_model_catalog_entry(
        &self,
        id: &str,
    ) -> Result<Option<ModelCatalogEntry>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, display_name, family, enabled, created_at
             FROM model_catalog
             WHERE id = ?1",
        )?;
        stmt.query_row(params![id], model_catalog_from_row)
            .optional()
    }

    pub async fn list_provider_model_routes(
        &self,
    ) -> Result<Vec<ProviderModelRoute>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, public_model_id, provider_account_id, upstream_model_id, wire_api,
                    role, enabled, status, last_error, last_status_code, cooldown_until,
                    last_used_at, strip_params, created_at
             FROM provider_model_routes
             ORDER BY public_model_id ASC,
                      CASE role WHEN 'primary' THEN 0 WHEN 'backup' THEN 1 ELSE 2 END,
                      created_at ASC",
        )?;
        let rows = stmt.query_map([], provider_model_route_from_row)?;
        rows.collect()
    }

    pub async fn create_provider_model_route(
        &self,
        input: CreateProviderModelRouteRequest,
    ) -> Result<ProviderModelRoute, rusqlite::Error> {
        let now = Utc::now();
        let route = ProviderModelRoute {
            id: Uuid::new_v4().to_string(),
            public_model_id: input.public_model_id.trim().to_string(),
            provider_account_id: input.provider_account_id.trim().to_string(),
            upstream_model_id: input.upstream_model_id.trim().to_string(),
            wire_api: normalize_wire_api(&input.wire_api, ""),
            role: normalize_route_role(&input.role),
            enabled: input.enabled,
            status: "healthy".to_string(),
            last_error: None,
            last_status_code: None,
            cooldown_until: None,
            last_used_at: None,
            strip_params: normalize_strip_params(input.strip_params),
            created_at: now,
        };
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO provider_model_routes
             (id, public_model_id, provider_account_id, upstream_model_id, wire_api, role,
              enabled, status, strip_params, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &route.id,
                &route.public_model_id,
                &route.provider_account_id,
                &route.upstream_model_id,
                &route.wire_api,
                &route.role,
                bool_to_i64(route.enabled),
                &route.status,
                serde_json::to_string(&route.strip_params).unwrap_or_else(|_| "[]".into()),
                route.created_at.to_rfc3339(),
            ],
        )?;
        Ok(route)
    }

    pub async fn update_provider_model_route(
        &self,
        id: &str,
        input: UpdateProviderModelRouteRequest,
    ) -> Result<ProviderModelRoute, rusqlite::Error> {
        let current = self
            .get_provider_model_route(id)
            .await?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let route = ProviderModelRoute {
            id: current.id,
            public_model_id: input
                .public_model_id
                .map(|value| value.trim().to_string())
                .unwrap_or(current.public_model_id),
            provider_account_id: input
                .provider_account_id
                .map(|value| value.trim().to_string())
                .unwrap_or(current.provider_account_id),
            upstream_model_id: input
                .upstream_model_id
                .map(|value| value.trim().to_string())
                .unwrap_or(current.upstream_model_id),
            wire_api: input
                .wire_api
                .map(|value| normalize_wire_api(&value, ""))
                .unwrap_or(current.wire_api),
            role: input
                .role
                .map(|value| normalize_route_role(&value))
                .unwrap_or(current.role),
            enabled: input.enabled.unwrap_or(current.enabled),
            status: current.status,
            last_error: current.last_error,
            last_status_code: current.last_status_code,
            cooldown_until: current.cooldown_until,
            last_used_at: current.last_used_at,
            strip_params: input
                .strip_params
                .map(normalize_strip_params)
                .unwrap_or(current.strip_params),
            created_at: current.created_at,
        };
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE provider_model_routes
             SET public_model_id = ?1, provider_account_id = ?2, upstream_model_id = ?3,
                 wire_api = ?4, role = ?5, enabled = ?6, strip_params = ?7
             WHERE id = ?8",
            params![
                &route.public_model_id,
                &route.provider_account_id,
                &route.upstream_model_id,
                &route.wire_api,
                &route.role,
                bool_to_i64(route.enabled),
                serde_json::to_string(&route.strip_params).unwrap_or_else(|_| "[]".into()),
                &route.id,
            ],
        )?;
        Ok(route)
    }

    pub async fn get_provider_model_route(
        &self,
        id: &str,
    ) -> Result<Option<ProviderModelRoute>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, public_model_id, provider_account_id, upstream_model_id, wire_api,
                    role, enabled, status, last_error, last_status_code, cooldown_until,
                    last_used_at, strip_params, created_at
             FROM provider_model_routes
             WHERE id = ?1",
        )?;
        stmt.query_row(params![id], provider_model_route_from_row)
            .optional()
    }

    pub async fn delete_provider_model_route(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let deleted = conn.execute(
            "DELETE FROM provider_model_routes WHERE id = ?1",
            params![id],
        )?;
        Ok(deleted > 0)
    }

    pub async fn list_routable_model_catalog(
        &self,
        wire_apis: &[&str],
    ) -> Result<Vec<RoutableModelCatalogEntry>, rusqlite::Error> {
        let entries = self.list_routable_model_catalog_entries().await?;
        let mut models = Vec::new();
        for model in entries {
            if !wire_apis.contains(&model.wire_api.as_str())
                || models
                    .iter()
                    .any(|existing: &RoutableModelCatalogEntry| existing.id == model.id)
            {
                continue;
            }
            models.push(model);
        }
        Ok(models)
    }

    pub async fn list_routable_model_catalog_by_wire(
        &self,
    ) -> Result<Vec<RoutableModelCatalogEntry>, rusqlite::Error> {
        let entries = self.list_routable_model_catalog_entries().await?;
        let mut models = Vec::new();
        for model in entries {
            if models.iter().any(|existing: &RoutableModelCatalogEntry| {
                existing.id == model.id && existing.wire_api == model.wire_api
            }) {
                continue;
            }
            models.push(model);
        }
        Ok(models)
    }

    async fn list_routable_model_catalog_entries(
        &self,
    ) -> Result<Vec<RoutableModelCatalogEntry>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT m.id, m.display_name, m.family, a.provider, r.wire_api, m.created_at
             FROM model_catalog m
             JOIN provider_model_routes r ON r.public_model_id = m.id
             JOIN provider_accounts a ON a.id = r.provider_account_id
             WHERE m.enabled = 1
               AND r.enabled = 1
               AND a.is_active = 1
               AND a.status != 'blocked'
               AND r.status != 'blocked'
               AND (r.cooldown_until IS NULL OR r.cooldown_until <= ?1)
             ORDER BY m.id ASC,
                      CASE r.role WHEN 'primary' THEN 0 WHEN 'backup' THEN 1 ELSE 2 END,
                      a.priority DESC,
                      COALESCE(r.last_used_at, a.last_used_at, '') ASC,
                      r.created_at ASC",
        )?;
        let rows = stmt.query_map(params![now], |row| {
            Ok(RoutableModelCatalogEntry {
                id: row.get(0)?,
                display_name: row.get(1)?,
                family: row.get(2)?,
                provider: row.get(3)?,
                wire_api: row.get(4)?,
                created_at: parse_time(row.get::<_, String>(5)?.as_str()),
            })
        })?;
        rows.collect()
    }

    pub async fn select_provider_account(
        &self,
        model: Option<&str>,
    ) -> Result<Option<ProviderRouteSelection>, rusqlite::Error> {
        self.select_provider_account_for_wire("anthropic-messages", model)
            .await
    }

    pub async fn select_provider_account_for_wire(
        &self,
        wire_api: &str,
        model: Option<&str>,
    ) -> Result<Option<ProviderRouteSelection>, rusqlite::Error> {
        let Some(model) = model else {
            return Ok(None);
        };
        let wire_api = normalize_wire_api(wire_api, "");
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.name, a.provider, a.base_url, a.auth_mode, a.wire_api, a.api_key,
                    a.is_active, a.priority, a.status, a.last_error, a.created_at, a.last_used_at,
                    r.id, r.public_model_id, r.upstream_model_id, r.strip_params
             FROM model_catalog m
             JOIN provider_model_routes r ON r.public_model_id = m.id
             JOIN provider_accounts a ON a.id = r.provider_account_id
             WHERE m.id = ?2
               AND m.enabled = 1
               AND r.wire_api = ?1
               AND r.enabled = 1
               AND a.is_active = 1
               AND a.status != 'blocked'
               AND r.status != 'blocked'
               AND (r.cooldown_until IS NULL OR r.cooldown_until <= ?3)
             ORDER BY CASE r.role WHEN 'primary' THEN 0 WHEN 'backup' THEN 1 ELSE 2 END,
                      a.priority DESC,
                      COALESCE(r.last_used_at, a.last_used_at, '') ASC,
                      r.created_at ASC
             LIMIT 1",
        )?;
        stmt.query_row(params![wire_api, model, now], route_selection_from_row)
            .optional()
    }

    pub async fn update_provider_account(
        &self,
        id: &str,
        input: UpdateProviderAccountRequest,
    ) -> Result<ProviderAccount, rusqlite::Error> {
        let current = self
            .get_provider_account(id)
            .await?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let provider = input
            .provider
            .map(|provider| normalize_provider(&provider))
            .unwrap_or(current.provider);
        let wire_api = input
            .wire_api
            .map(|wire_api| normalize_wire_api(&wire_api, &provider))
            .unwrap_or(current.wire_api);
        let auth_mode = input
            .auth_mode
            .as_deref()
            .map(normalize_auth_mode)
            .unwrap_or_else(|| current.auth_mode.clone());
        let base_url = input
            .base_url
            .as_deref()
            .map(|url| normalize_provider_base_url(url, &auth_mode))
            .unwrap_or_else(|| normalize_provider_base_url(&current.base_url, &auth_mode));
        let account = ProviderAccount {
            id: current.id,
            name: input.name.unwrap_or(current.name),
            provider,
            base_url,
            auth_mode,
            wire_api,
            is_active: input.is_active.unwrap_or(current.is_active),
            priority: input.priority.unwrap_or(current.priority),
            status: "healthy".to_string(),
            last_error: None,
            created_at: current.created_at,
            last_used_at: current.last_used_at,
        };

        let conn = self.conn.lock().await;
        if let Some(api_key) = input.api_key {
            conn.execute(
                "UPDATE provider_accounts SET name = ?1, provider = ?2, base_url = ?3,
                 auth_mode = ?4, wire_api = ?5, api_key = ?6, is_active = ?7, priority = ?8,
                 status = 'healthy', last_error = NULL WHERE id = ?9",
                params![
                    &account.name,
                    &account.provider,
                    &account.base_url,
                    &account.auth_mode,
                    &account.wire_api,
                    api_key,
                    bool_to_i64(account.is_active),
                    account.priority,
                    &account.id,
                ],
            )?;
        } else {
            conn.execute(
                "UPDATE provider_accounts SET name = ?1, provider = ?2, base_url = ?3,
                 auth_mode = ?4, wire_api = ?5, is_active = ?6, priority = ?7,
                 status = 'healthy', last_error = NULL WHERE id = ?8",
                params![
                    &account.name,
                    &account.provider,
                    &account.base_url,
                    &account.auth_mode,
                    &account.wire_api,
                    bool_to_i64(account.is_active),
                    account.priority,
                    &account.id,
                ],
            )?;
        }
        Ok(account)
    }

    pub async fn get_provider_account(
        &self,
        id: &str,
    ) -> Result<Option<ProviderAccount>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, base_url, auth_mode, wire_api, api_key, is_active,
                    priority, status, last_error, created_at, last_used_at
             FROM provider_accounts WHERE id = ?1",
        )?;
        stmt.query_row(params![id], account_from_row)
            .optional()
            .map(|record| record.map(|record| record.account))
    }

    pub async fn get_provider_account_record(
        &self,
        id: &str,
    ) -> Result<Option<ProviderAccountRecord>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, base_url, auth_mode, wire_api, api_key, is_active,
                    priority, status, last_error, created_at, last_used_at
             FROM provider_accounts WHERE id = ?1",
        )?;
        stmt.query_row(params![id], account_from_row).optional()
    }

    pub async fn update_provider_account_secret(
        &self,
        id: &str,
        api_key: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE provider_accounts SET api_key = ?1 WHERE id = ?2",
            params![api_key, id],
        )?;
        Ok(())
    }

    pub async fn delete_provider_account(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM provider_accounts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub async fn mark_provider_result(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE provider_accounts SET status = ?1, last_error = ?2, last_used_at = ?3 WHERE id = ?4",
            params![status, error, Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub async fn mark_route_success(
        &self,
        id: &str,
        status_code: u16,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE provider_model_routes
             SET status = 'healthy',
                 last_error = NULL,
                 last_status_code = ?1,
                 cooldown_until = NULL,
                 last_used_at = ?2
             WHERE id = ?3",
            params![status_code, Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub async fn mark_route_failure(
        &self,
        id: &str,
        status: &str,
        last_status_code: Option<u16>,
        error: &str,
        cooldown_until: Option<DateTime<Utc>>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE provider_model_routes
             SET status = ?1,
                 last_error = ?2,
                 last_status_code = ?3,
                 cooldown_until = ?4,
                 last_used_at = ?5
             WHERE id = ?6",
            params![
                status,
                error,
                last_status_code.map(i64::from),
                cooldown_until.map(|value| value.to_rfc3339()),
                Utc::now().to_rfc3339(),
                id,
            ],
        )?;
        Ok(())
    }

    pub async fn insert_request_log(&self, log: RequestLog) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO request_logs
             (id, api_key_id, provider_account_id, method, path, model, upstream_model,
              upstream_url, request_summary, status_code, latency_ms, input_tokens,
              cached_input_tokens, output_tokens, cost_usd, created_at, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                log.id,
                log.api_key_id,
                log.provider_account_id,
                log.method,
                log.path,
                log.model,
                log.upstream_model,
                log.upstream_url,
                log.request_summary
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .unwrap_or_default(),
                log.status_code,
                log.latency_ms,
                log.input_tokens,
                log.cached_input_tokens,
                log.output_tokens,
                log.cost_usd,
                log.created_at.to_rfc3339(),
                log.error,
            ],
        )?;
        Ok(())
    }

    pub async fn list_request_logs(&self, limit: u32) -> Result<Vec<RequestLog>, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, api_key_id, provider_account_id, method, path, model, upstream_model,
                    upstream_url, request_summary, status_code, latency_ms, input_tokens,
                    cached_input_tokens, output_tokens, cost_usd, created_at, error
             FROM request_logs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], request_log_from_row)?;
        rows.collect()
    }

    pub async fn dashboard(&self) -> Result<Dashboard, rusqlite::Error> {
        let conn = self.conn.lock().await;
        let total_api_keys = count(&conn, "SELECT COUNT(*) FROM api_keys")?;
        let active_api_keys = count(&conn, "SELECT COUNT(*) FROM api_keys WHERE is_active = 1")?;
        let total_accounts = count(&conn, "SELECT COUNT(*) FROM provider_accounts")?;
        let healthy_accounts = count(
            &conn,
            "SELECT COUNT(*) FROM provider_accounts WHERE is_active = 1 AND status = 'healthy'",
        )?;
        let today = Utc::now().date_naive();
        let usage = usage_summary(&conn, today)?;
        let request_trend = request_trend(&conn, Utc::now())?;
        drop(conn);

        Ok(Dashboard {
            active_api_keys,
            total_api_keys,
            healthy_accounts,
            total_accounts,
            usage,
            accounts: self.list_provider_accounts().await?,
            recent_requests: self.list_request_logs(10).await?,
            request_trend,
        })
    }
}

fn rows_to_api_keys<P>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
) -> Result<Vec<ApiKeyView>, rusqlite::Error>
where
    P: rusqlite::Params,
{
    let rows = stmt.query_map(params, api_key_from_row)?;
    rows.map(|row| row.map(|record| record.view)).collect()
}

fn ensure_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
    column_definition: &str,
) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let existing: String = row.get(1)?;
        if existing == column_name {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition}"),
        [],
    )?;
    Ok(())
}

fn migrate_legacy_codex_base_urls(conn: &mut Connection) -> Result<usize, rusqlite::Error> {
    let transaction = conn.transaction()?;
    let candidates = {
        let mut statement = transaction.prepare(
            "SELECT id, base_url FROM provider_accounts WHERE auth_mode = 'codex-oauth'",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let mut migrated = 0;
    for (id, base_url) in candidates {
        let Some(base_url) = canonicalize_legacy_codex_base_url(&base_url) else {
            continue;
        };
        migrated += transaction.execute(
            "UPDATE provider_accounts SET base_url = ?1 WHERE id = ?2",
            params![base_url, id],
        )?;
    }
    transaction.commit()?;
    Ok(migrated)
}

fn rows_to_accounts<P>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
) -> Result<Vec<ProviderAccount>, rusqlite::Error>
where
    P: rusqlite::Params,
{
    let rows = stmt.query_map(params, account_from_row)?;
    rows.map(|row| row.map(|record| record.account)).collect()
}

fn api_key_from_row(row: &rusqlite::Row<'_>) -> Result<ApiKeyRecord, rusqlite::Error> {
    let permissions: String = row.get(6)?;
    Ok(ApiKeyRecord {
        view: ApiKeyView {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            key_preview: row.get(4)?,
            is_active: row.get::<_, i64>(5)? == 1,
            permissions: serde_json::from_str(&permissions).unwrap_or_default(),
            rate_limit_per_minute: row.get::<_, i64>(7)? as u32,
            concurrency_limit: row.get::<_, i64>(8)? as u32,
            daily_cost_limit: row.get(9)?,
            expires_at: parse_time_opt(row.get::<_, Option<String>>(10)?.as_deref()),
            created_at: parse_time(row.get::<_, String>(11)?.as_str()),
            last_used_at: parse_time_opt(row.get::<_, Option<String>>(12)?.as_deref()),
        },
        key_hash: row.get(3)?,
    })
}

fn account_from_row(row: &rusqlite::Row<'_>) -> Result<ProviderAccountRecord, rusqlite::Error> {
    Ok(ProviderAccountRecord {
        account: ProviderAccount {
            id: row.get(0)?,
            name: row.get(1)?,
            provider: row.get(2)?,
            base_url: row.get(3)?,
            auth_mode: row.get(4)?,
            wire_api: row.get(5)?,
            is_active: row.get::<_, i64>(7)? == 1,
            priority: row.get(8)?,
            status: row.get(9)?,
            last_error: row.get(10)?,
            created_at: parse_time(row.get::<_, String>(11)?.as_str()),
            last_used_at: parse_time_opt(row.get::<_, Option<String>>(12)?.as_deref()),
        },
        api_key: row.get(6)?,
    })
}

fn model_catalog_from_row(row: &rusqlite::Row<'_>) -> Result<ModelCatalogEntry, rusqlite::Error> {
    Ok(ModelCatalogEntry {
        id: row.get(0)?,
        display_name: row.get(1)?,
        family: row.get(2)?,
        enabled: row.get::<_, i64>(3)? == 1,
        created_at: parse_time(row.get::<_, String>(4)?.as_str()),
    })
}

fn provider_model_route_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<ProviderModelRoute, rusqlite::Error> {
    let strip_params: String = row.get(12)?;
    Ok(ProviderModelRoute {
        id: row.get(0)?,
        public_model_id: row.get(1)?,
        provider_account_id: row.get(2)?,
        upstream_model_id: row.get(3)?,
        wire_api: row.get(4)?,
        role: row.get(5)?,
        enabled: row.get::<_, i64>(6)? == 1,
        status: row.get(7)?,
        last_error: row.get(8)?,
        last_status_code: row.get::<_, Option<i64>>(9)?.map(|value| value as u16),
        cooldown_until: parse_time_opt(row.get::<_, Option<String>>(10)?.as_deref()),
        last_used_at: parse_time_opt(row.get::<_, Option<String>>(11)?.as_deref()),
        strip_params: serde_json::from_str(&strip_params).unwrap_or_default(),
        created_at: parse_time(row.get::<_, String>(13)?.as_str()),
    })
}

fn route_selection_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<ProviderRouteSelection, rusqlite::Error> {
    let strip_params: String = row.get(16)?;
    Ok(ProviderRouteSelection {
        account: account_from_row(row)?,
        route_id: row.get(13)?,
        public_model_id: row.get(14)?,
        upstream_model_id: row.get(15)?,
        strip_params: serde_json::from_str(&strip_params).unwrap_or_default(),
    })
}

fn request_log_from_row(row: &rusqlite::Row<'_>) -> Result<RequestLog, rusqlite::Error> {
    let request_summary: Option<String> = row.get(8)?;
    Ok(RequestLog {
        id: row.get(0)?,
        api_key_id: row.get(1)?,
        provider_account_id: row.get(2)?,
        method: row.get(3)?,
        path: row.get(4)?,
        model: row.get(5)?,
        upstream_model: row.get(6)?,
        upstream_url: row.get(7)?,
        request_summary: request_summary
            .as_deref()
            .and_then(|value| serde_json::from_str::<RequestSummary>(value).ok()),
        status_code: row.get::<_, i64>(9)? as u16,
        latency_ms: row.get::<_, i64>(10)? as u64,
        input_tokens: row.get::<_, i64>(11)? as u64,
        cached_input_tokens: row.get::<_, i64>(12)? as u64,
        output_tokens: row.get::<_, i64>(13)? as u64,
        cost_usd: row.get(14)?,
        created_at: parse_time(row.get::<_, String>(15)?.as_str()),
        error: row.get(16)?,
    })
}

fn count(conn: &Connection, sql: &str) -> Result<u64, rusqlite::Error> {
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .map(|value| value as u64)
}

fn usage_summary(conn: &Connection, today: NaiveDate) -> Result<UsageSummary, rusqlite::Error> {
    let prefix = today.to_string();
    let (
        requests_today,
        tokens_today,
        input_tokens_today,
        cached_input_tokens_today,
        estimated_cost_today,
    ) = conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(input_tokens + output_tokens), 0),
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(cached_input_tokens), 0),
                COALESCE(SUM(cost_usd), 0)
         FROM request_logs
         WHERE created_at LIKE ?1 || '%'",
        params![prefix],
        |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)? as u64,
                row.get::<_, i64>(3)? as u64,
                row.get::<_, f64>(4)?,
            ))
        },
    )?;
    let (total_requests, total_tokens) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(input_tokens + output_tokens), 0) FROM request_logs",
        [],
        |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
    )?;

    Ok(UsageSummary {
        requests_today,
        tokens_today,
        input_tokens_today,
        cached_input_tokens_today,
        total_requests,
        total_tokens,
        estimated_cost_today,
    })
}

const REQUEST_TREND_BUCKET_COUNT: i64 = 12;
const REQUEST_TREND_BUCKET_SECONDS: i64 = 5 * 60;

fn request_trend(conn: &Connection, now: DateTime<Utc>) -> Result<RequestTrend, rusqlite::Error> {
    let current_bucket_started_at =
        now.timestamp().div_euclid(REQUEST_TREND_BUCKET_SECONDS) * REQUEST_TREND_BUCKET_SECONDS;
    let window_started_at =
        current_bucket_started_at - (REQUEST_TREND_BUCKET_COUNT - 1) * REQUEST_TREND_BUCKET_SECONDS;
    let window_ended_at = current_bucket_started_at + REQUEST_TREND_BUCKET_SECONDS;
    let window_started_at_text = DateTime::from_timestamp(window_started_at, 0)
        .expect("request trend start is in range")
        .to_rfc3339();
    let window_ended_at_text = DateTime::from_timestamp(window_ended_at, 0)
        .expect("request trend end is in range")
        .to_rfc3339();
    let mut buckets = (0..REQUEST_TREND_BUCKET_COUNT)
        .map(|index| RequestTrendBucket {
            started_at: DateTime::from_timestamp(
                window_started_at + index * REQUEST_TREND_BUCKET_SECONDS,
                0,
            )
            .expect("request trend timestamp is in range"),
            request_count: 0,
        })
        .collect::<Vec<_>>();

    let mut stmt = conn.prepare(
        "SELECT (unixepoch(created_at) - ?1) / ?2 AS bucket_index, COUNT(*)
         FROM request_logs
         WHERE created_at >= ?3 AND created_at < ?4
         GROUP BY bucket_index
         ORDER BY bucket_index",
    )?;
    let rows = stmt.query_map(
        params![
            window_started_at,
            REQUEST_TREND_BUCKET_SECONDS,
            window_started_at_text,
            window_ended_at_text
        ],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    for row in rows {
        let (index, count) = row?;
        if let Some(bucket) = usize::try_from(index)
            .ok()
            .and_then(|index| buckets.get_mut(index))
        {
            bucket.request_count = count as u64;
        }
    }

    Ok(RequestTrend {
        window_started_at: DateTime::from_timestamp(window_started_at, 0)
            .expect("request trend start is in range"),
        window_ended_at: DateTime::from_timestamp(window_ended_at, 0)
            .expect("request trend end is in range"),
        bucket_duration_seconds: REQUEST_TREND_BUCKET_SECONDS as u64,
        buckets,
    })
}

fn parse_time(value: &str) -> DateTime<Utc> {
    parse_time_opt(Some(value)).unwrap_or_else(Utc::now)
}

fn parse_time_opt(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

fn bool_to_i64(value: bool) -> i64 {
    i64::from(value)
}

fn normalize_provider(value: &str) -> String {
    normalize_provider_alias(value)
}

fn normalize_auth_mode(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "bearer" | "authorization" => "bearer".to_string(),
        "x-goog-api-key" | "google-api-key" | "goog-api-key" | "gemini-api-key" => {
            "x-goog-api-key".to_string()
        }
        "codex" | "codex-oauth" | "chatgpt" | "chatgpt-oauth" => "codex-oauth".to_string(),
        "antigravity" | "antigravity-oauth" | "google-antigravity" => {
            "antigravity-oauth".to_string()
        }
        _ => "x-api-key".to_string(),
    }
}

fn normalize_wire_api(value: &str, provider: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "anthropic" | "anthropic-messages" | "messages" | "claude" => {
            "anthropic-messages".to_string()
        }
        "openai" | "openai-chat" | "chat" | "chat-completions" => "openai-chat".to_string(),
        "openai-responses" | "responses" | "codex" => "openai-responses".to_string(),
        "gemini" | "gemini-generate-content" | "generate-content" | "generatecontent" => {
            "gemini-generate-content".to_string()
        }
        "" => default_wire_api_for_provider(provider)
            .unwrap_or("anthropic-messages")
            .to_string(),
        _ => "anthropic-messages".to_string(),
    }
}

fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn normalize_provider_base_url(value: &str, auth_mode: &str) -> String {
    let base_url = normalize_base_url(value);
    if is_codex_subscription_auth(auth_mode) {
        canonicalize_legacy_codex_base_url(&base_url).unwrap_or(base_url)
    } else {
        base_url
    }
}

fn normalize_family(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "" => "other".to_string(),
        "anthropic" | "claude" => "anthropic".to_string(),
        "openai" | "gpt" | "codex" | "codex-subscription" | "chatgpt" => "openai".to_string(),
        "google" | "google-ai" | "google-ai-studio" | "gemini" | "gemini-code-assist"
        | "cloudcode" => "gemini".to_string(),
        "z.ai" | "zai" | "zhipu" | "zhipuai" => "glm".to_string(),
        other => other.to_string(),
    }
}

fn normalize_route_role(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "backup" | "secondary" | "fallback" => "backup".to_string(),
        _ => "primary".to_string(),
    }
}

fn normalize_strip_params(values: Vec<String>) -> Vec<String> {
    let mut params = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() || params.iter().any(|existing| existing == value) {
            continue;
        }
        params.push(value.to_string());
    }
    params
}

fn default_display_name(value: String, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn usage_summary_tracks_cached_input_tokens_today() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");

        {
            let conn = db.conn.lock().await;
            let empty = usage_summary(
                &conn,
                NaiveDate::from_ymd_opt(2026, 7, 19).expect("valid date"),
            )
            .expect("build empty usage summary");
            assert_eq!(empty.requests_today, 0);
            assert_eq!(empty.tokens_today, 0);
            assert_eq!(empty.input_tokens_today, 0);
            assert_eq!(empty.cached_input_tokens_today, 0);
        }

        for (id, timestamp, input_tokens, cached_input_tokens, output_tokens) in [
            ("today", "2026-07-19T04:00:00Z", 100, 60, 20),
            ("yesterday", "2026-07-18T04:00:00Z", 40, 40, 10),
        ] {
            db.insert_request_log(RequestLog {
                id: id.to_string(),
                api_key_id: "test-key".to_string(),
                provider_account_id: None,
                method: "POST".to_string(),
                path: "/openai/v1/responses".to_string(),
                model: Some("test-model".to_string()),
                upstream_model: None,
                upstream_url: None,
                request_summary: None,
                status_code: 200,
                latency_ms: 10,
                input_tokens,
                cached_input_tokens,
                output_tokens,
                cost_usd: 0.0,
                created_at: DateTime::parse_from_rfc3339(timestamp)
                    .expect("parse request timestamp")
                    .with_timezone(&Utc),
                error: None,
            })
            .await
            .expect("insert request log");
        }

        let conn = db.conn.lock().await;
        let summary = usage_summary(
            &conn,
            NaiveDate::from_ymd_opt(2026, 7, 19).expect("valid date"),
        )
        .expect("build usage summary");
        drop(conn);

        assert_eq!(summary.requests_today, 1);
        assert_eq!(summary.tokens_today, 120);
        assert_eq!(summary.input_tokens_today, 100);
        assert_eq!(summary.cached_input_tokens_today, 60);
        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.total_tokens, 170);

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn startup_adds_cached_tokens_to_legacy_request_logs() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let conn = Connection::open(&path).expect("open legacy database");
        conn.execute_batch(
            r#"
            CREATE TABLE request_logs (
                id TEXT PRIMARY KEY,
                api_key_id TEXT NOT NULL,
                provider_account_id TEXT,
                method TEXT NOT NULL,
                path TEXT NOT NULL,
                model TEXT,
                upstream_model TEXT,
                upstream_url TEXT,
                request_summary TEXT,
                status_code INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                error TEXT
            );
            INSERT INTO request_logs (
                id, api_key_id, method, path, status_code, latency_ms,
                input_tokens, output_tokens, created_at
            ) VALUES (
                'legacy-request', 'legacy-key', 'POST', '/openai/v1/responses',
                200, 10, 12, 4, '2026-07-19T04:00:00+00:00'
            );
            "#,
        )
        .expect("seed legacy request log");
        drop(conn);

        let db = Db::open(&path).await.expect("migrate legacy database");
        let logs = db.list_request_logs(1).await.expect("read migrated log");

        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].input_tokens, 12);
        assert_eq!(logs[0].cached_input_tokens, 0);
        assert_eq!(logs[0].output_tokens, 4);

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn request_trend_counts_real_five_minute_intervals() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");
        let now = DateTime::parse_from_rfc3339("2026-07-15T02:02:00Z")
            .expect("parse now")
            .with_timezone(&Utc);

        for (index, timestamp) in [
            "2026-07-15T01:04:59Z",
            "2026-07-15T01:05:00Z",
            "2026-07-15T01:09:59Z",
            "2026-07-15T01:10:00Z",
            "2026-07-15T01:35:00Z",
            "2026-07-15T02:00:00Z",
            "2026-07-15T02:02:00Z",
            "2026-07-15T02:05:00Z",
        ]
        .into_iter()
        .enumerate()
        {
            db.insert_request_log(RequestLog {
                id: format!("request-{index}"),
                api_key_id: "test-key".to_string(),
                provider_account_id: None,
                method: "POST".to_string(),
                path: "/openai/v1/responses".to_string(),
                model: Some("test-model".to_string()),
                upstream_model: None,
                upstream_url: None,
                request_summary: None,
                status_code: 200,
                latency_ms: 10,
                input_tokens: 1,
                cached_input_tokens: 0,
                output_tokens: 1,
                cost_usd: 0.0,
                created_at: DateTime::parse_from_rfc3339(timestamp)
                    .expect("parse request timestamp")
                    .with_timezone(&Utc),
                error: None,
            })
            .await
            .expect("insert request log");
        }

        let conn = db.conn.lock().await;
        let trend = request_trend(&conn, now).expect("build request trend");
        drop(conn);

        assert_eq!(trend.bucket_duration_seconds, 300);
        assert_eq!(trend.buckets.len(), 12);
        assert_eq!(
            trend.window_started_at.to_rfc3339(),
            "2026-07-15T01:05:00+00:00"
        );
        assert_eq!(
            trend.window_ended_at.to_rfc3339(),
            "2026-07-15T02:05:00+00:00"
        );
        assert_eq!(trend.buckets[0].request_count, 2);
        assert_eq!(trend.buckets[1].request_count, 1);
        assert_eq!(trend.buckets[6].request_count, 1);
        assert_eq!(trend.buckets[11].request_count, 2);
        assert_eq!(
            trend
                .buckets
                .iter()
                .map(|bucket| bucket.request_count)
                .sum::<u64>(),
            6
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn primary_route_wins_and_rewrites_upstream_model() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");

        let backup = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Backup".to_string(),
                provider: "openai-compatible".to_string(),
                base_url: "https://backup.example.com/".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-chat".to_string(),
                api_key: "backup-key".to_string(),
                is_active: true,
                priority: 100,
            })
            .await
            .expect("create backup account");
        let primary = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Primary".to_string(),
                provider: "minimax".to_string(),
                base_url: "https://minimax.example.com/".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-chat".to_string(),
                api_key: "primary-key".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create primary account");
        db.create_model_catalog_entry(CreateModelCatalogEntryRequest {
            id: "MiniMax-M3".to_string(),
            display_name: String::new(),
            family: "minimax".to_string(),
            enabled: true,
        })
        .await
        .expect("create catalog model");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "MiniMax-M3".to_string(),
            provider_account_id: backup.id,
            upstream_model_id: "backup-minimax-m3".to_string(),
            wire_api: "openai-chat".to_string(),
            role: "backup".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create backup route");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "MiniMax-M3".to_string(),
            provider_account_id: primary.id,
            upstream_model_id: "MiniMax-M3".to_string(),
            wire_api: "openai-chat".to_string(),
            role: "primary".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create primary route");

        let selected = db
            .select_provider_account_for_wire("openai-chat", Some("MiniMax-M3"))
            .await
            .expect("select provider")
            .expect("selected provider");

        assert_eq!(selected.account.account.provider, "minimax");
        assert_eq!(
            selected.account.account.base_url,
            "https://minimax.example.com"
        );
        assert_eq!(selected.public_model_id, "MiniMax-M3");
        assert_eq!(selected.upstream_model_id, "MiniMax-M3");
        assert!(
            db.select_provider_account_for_wire("openai-chat", Some("minimax-m3"))
                .await
                .expect("select lowercase")
                .is_none()
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn backup_route_is_selected_after_primary_is_blocked() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");

        let primary = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Primary".to_string(),
                provider: "deepseek".to_string(),
                base_url: "https://primary.example.com/".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-chat".to_string(),
                api_key: "primary-key".to_string(),
                is_active: true,
                priority: 100,
            })
            .await
            .expect("create primary account");
        let backup = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Backup".to_string(),
                provider: "deepseek".to_string(),
                base_url: "https://backup.example.com/".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-chat".to_string(),
                api_key: "backup-key".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create backup account");
        db.create_model_catalog_entry(CreateModelCatalogEntryRequest {
            id: "deepseek-v4-pro".to_string(),
            display_name: String::new(),
            family: "deepseek".to_string(),
            enabled: true,
        })
        .await
        .expect("create catalog model");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "deepseek-v4-pro".to_string(),
            provider_account_id: primary.id.clone(),
            upstream_model_id: "deepseek-v4-pro".to_string(),
            wire_api: "openai-chat".to_string(),
            role: "primary".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create primary route");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "deepseek-v4-pro".to_string(),
            provider_account_id: backup.id,
            upstream_model_id: "deepseek-v4-pro-backup".to_string(),
            wire_api: "openai-chat".to_string(),
            role: "backup".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create backup route");

        db.mark_provider_result(&primary.id, "blocked", Some("401"))
            .await
            .expect("mark primary blocked");

        let selected = db
            .select_provider_account_for_wire("openai-chat", Some("deepseek-v4-pro"))
            .await
            .expect("select provider")
            .expect("selected provider");

        assert_eq!(selected.account.account.name, "Backup");
        assert_eq!(selected.upstream_model_id, "deepseek-v4-pro-backup");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn backup_route_is_selected_after_primary_route_cools_down() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");

        let primary = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Primary".to_string(),
                provider: "deepseek".to_string(),
                base_url: "https://primary.example.com/".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-chat".to_string(),
                api_key: "primary-key".to_string(),
                is_active: true,
                priority: 100,
            })
            .await
            .expect("create primary account");
        let backup = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Backup".to_string(),
                provider: "deepseek".to_string(),
                base_url: "https://backup.example.com/".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-chat".to_string(),
                api_key: "backup-key".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create backup account");
        db.create_model_catalog_entry(CreateModelCatalogEntryRequest {
            id: "deepseek-v4-flash".to_string(),
            display_name: String::new(),
            family: "deepseek".to_string(),
            enabled: true,
        })
        .await
        .expect("create catalog model");
        let primary_route = db
            .create_provider_model_route(CreateProviderModelRouteRequest {
                public_model_id: "deepseek-v4-flash".to_string(),
                provider_account_id: primary.id.clone(),
                upstream_model_id: "deepseek-v4-flash".to_string(),
                wire_api: "openai-chat".to_string(),
                role: "primary".to_string(),
                enabled: true,
                strip_params: vec!["temperature".to_string()],
            })
            .await
            .expect("create primary route");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "deepseek-v4-flash".to_string(),
            provider_account_id: backup.id,
            upstream_model_id: "deepseek-v4-flash-backup".to_string(),
            wire_api: "openai-chat".to_string(),
            role: "backup".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create backup route");

        let selected = db
            .select_provider_account_for_wire("openai-chat", Some("deepseek-v4-flash"))
            .await
            .expect("select primary")
            .expect("primary selected");
        assert_eq!(selected.route_id, primary_route.id);
        assert_eq!(selected.strip_params, vec!["temperature"]);

        db.mark_route_failure(
            &primary_route.id,
            "cooling_down",
            Some(429),
            "upstream returned 429",
            Some(Utc::now() + chrono::Duration::seconds(60)),
        )
        .await
        .expect("mark route cooling down");

        let selected = db
            .select_provider_account_for_wire("openai-chat", Some("deepseek-v4-flash"))
            .await
            .expect("select backup")
            .expect("backup selected");

        assert_eq!(selected.account.account.name, "Backup");
        assert_eq!(selected.upstream_model_id, "deepseek-v4-flash-backup");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn openai_compatible_provider_aliases_default_to_chat() {
        for provider in [
            "deepseek",
            "qwen",
            "dashscope",
            "kimi",
            "kimi-for-coding",
            "moonshot",
            "moonshotai",
            "moonshotai-cn",
            "Z. AI",
            "zai-coding-plan",
            "Zhipu. AI",
            "zhipuai-coding-plan",
            "glm",
        ] {
            assert_eq!(normalize_wire_api("", provider), "openai-chat");
        }
        assert_eq!(normalize_provider("kimi"), "kimi-for-coding");
        assert_eq!(normalize_provider("moonshot"), "moonshotai");
        assert_eq!(normalize_provider("Z. AI"), "zai");
        assert_eq!(normalize_provider("Zhipu. AI"), "zhipuai");
    }

    #[test]
    fn minimax_provider_aliases_default_to_anthropic_messages() {
        for provider in [
            "minimax",
            "MiniMax Token Plan",
            "minimax-coding-plan",
            "minimax-cn",
            "MiniMax CN Token Plan",
            "minimax-cn-coding-plan",
        ] {
            assert_eq!(normalize_wire_api("", provider), "anthropic-messages");
        }
        assert_eq!(
            normalize_provider("MiniMax Token Plan"),
            "minimax-coding-plan"
        );
        assert_eq!(
            normalize_provider("MiniMax CN Token Plan"),
            "minimax-cn-coding-plan"
        );
    }

    #[test]
    fn codex_subscription_aliases_default_to_responses() {
        assert_eq!(normalize_provider("codex"), "codex-subscription");
        assert_eq!(normalize_auth_mode("chatgpt-oauth"), "codex-oauth");
        assert_eq!(
            normalize_wire_api("", "codex-subscription"),
            "openai-responses"
        );
    }

    #[tokio::test]
    async fn codex_account_writes_canonicalize_legacy_base_urls() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");
        let account = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Codex".to_string(),
                provider: "codex-subscription".to_string(),
                base_url: "https://relay.example/backend-api/codex/".to_string(),
                auth_mode: "codex-oauth".to_string(),
                wire_api: "openai-responses".to_string(),
                api_key: "refresh-token".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create Codex account");
        assert_eq!(account.base_url, "https://relay.example/backend-api");
        db.mark_provider_result(&account.id, "blocked", Some("expired credential"))
            .await
            .expect("block Codex account");

        let account = db
            .update_provider_account(
                &account.id,
                UpdateProviderAccountRequest {
                    name: None,
                    provider: None,
                    base_url: Some(
                        "https://relay.example/tenant/backend-api/codex/responses".to_string(),
                    ),
                    auth_mode: None,
                    wire_api: None,
                    api_key: None,
                    is_active: None,
                    priority: None,
                },
            )
            .await
            .expect("update Codex account");
        assert_eq!(account.base_url, "https://relay.example/tenant/backend-api");
        assert_eq!(account.status, "healthy");
        assert!(account.last_error.is_none());

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn startup_migrates_only_legacy_codex_account_base_urls() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");
        let legacy_codex = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Legacy Codex".to_string(),
                provider: "codex-subscription".to_string(),
                base_url: "https://relay.example/backend-api".to_string(),
                auth_mode: "codex-oauth".to_string(),
                wire_api: "openai-responses".to_string(),
                api_key: "refresh-token-1".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create first Codex account");
        let legacy_responses = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Legacy Responses".to_string(),
                provider: "codex-subscription".to_string(),
                base_url: "https://relay.example/tenant/backend-api".to_string(),
                auth_mode: "codex-oauth".to_string(),
                wire_api: "openai-responses".to_string(),
                api_key: "refresh-token-2".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create second Codex account");
        let bearer = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Bearer".to_string(),
                provider: "openai".to_string(),
                base_url: "https://relay.example/backend-api/codex".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-responses".to_string(),
                api_key: "api-key".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create bearer account");
        {
            let conn = db.conn.lock().await;
            conn.execute(
                "UPDATE provider_accounts SET base_url = ?1 WHERE id = ?2",
                rusqlite::params!["https://relay.example/backend-api/codex", legacy_codex.id],
            )
            .expect("seed legacy Codex URL");
            conn.execute(
                "UPDATE provider_accounts SET base_url = ?1 WHERE id = ?2",
                rusqlite::params![
                    "https://relay.example/tenant/backend-api/codex/responses",
                    legacy_responses.id
                ],
            )
            .expect("seed legacy responses URL");
        }
        drop(db);

        let db = Db::open(&path).await.expect("reopen migrated database");
        assert_eq!(
            db.get_provider_account(&legacy_codex.id)
                .await
                .expect("load first Codex account")
                .expect("first Codex account")
                .base_url,
            "https://relay.example/backend-api"
        );
        assert_eq!(
            db.get_provider_account(&legacy_responses.id)
                .await
                .expect("load second Codex account")
                .expect("second Codex account")
                .base_url,
            "https://relay.example/tenant/backend-api"
        );
        assert_eq!(
            db.get_provider_account(&bearer.id)
                .await
                .expect("load bearer account")
                .expect("bearer account")
                .base_url,
            "https://relay.example/backend-api/codex"
        );

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn gemini_aliases_default_to_generate_content() {
        for provider in ["gemini", "google", "Google AI", "google-ai-studio"] {
            assert_eq!(normalize_wire_api("", provider), "gemini-generate-content");
        }
        assert_eq!(normalize_provider("Google AI"), "gemini");
        assert_eq!(normalize_auth_mode("google-api-key"), "x-goog-api-key");
        assert_eq!(
            normalize_auth_mode("google-antigravity"),
            "antigravity-oauth"
        );
        assert_eq!(
            normalize_wire_api("generateContent", "gemini"),
            "gemini-generate-content"
        );
    }

    #[tokio::test]
    async fn delete_api_key_reports_missing_rows() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");
        let key = db
            .create_api_key(
                CreateApiKeyRequest {
                    name: "Client".to_string(),
                    description: String::new(),
                    permissions: Vec::new(),
                    rate_limit_per_minute: 0,
                    concurrency_limit: 0,
                    daily_cost_limit: 0.0,
                    expires_at: None,
                },
                "tokentoxication-test-secret",
            )
            .await
            .expect("create api key");

        assert!(db.delete_api_key(&key.id).await.expect("delete api key"));
        assert!(
            db.get_api_key(&key.id)
                .await
                .expect("get api key")
                .is_none()
        );
        assert!(
            !db.delete_api_key(&key.id)
                .await
                .expect("delete missing api key")
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn model_catalog_lists_enabled_routable_models_for_wire_api() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");

        let deepseek = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "DeepSeek".to_string(),
                provider: "deepseek".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-chat".to_string(),
                api_key: "deepseek-key".to_string(),
                is_active: true,
                priority: 10,
            })
            .await
            .expect("create deepseek account");
        let duplicate = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Duplicate".to_string(),
                provider: "openai".to_string(),
                base_url: "https://api.openai.com".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-responses".to_string(),
                api_key: "openai-key".to_string(),
                is_active: true,
                priority: 0,
            })
            .await
            .expect("create duplicate account");
        let inactive = db
            .create_provider_account(CreateProviderAccountRequest {
                name: "Inactive".to_string(),
                provider: "openai".to_string(),
                base_url: "https://api.openai.com".to_string(),
                auth_mode: "bearer".to_string(),
                wire_api: "openai-responses".to_string(),
                api_key: "openai-key".to_string(),
                is_active: false,
                priority: 100,
            })
            .await
            .expect("create inactive account");
        db.create_model_catalog_entry(CreateModelCatalogEntryRequest {
            id: "deepseek-v4-pro".to_string(),
            display_name: "DeepSeek V4 Pro".to_string(),
            family: "deepseek".to_string(),
            enabled: true,
        })
        .await
        .expect("create deepseek model");
        db.create_model_catalog_entry(CreateModelCatalogEntryRequest {
            id: "gpt-5.5".to_string(),
            display_name: String::new(),
            family: "openai".to_string(),
            enabled: true,
        })
        .await
        .expect("create inactive model");
        db.create_model_catalog_entry(CreateModelCatalogEntryRequest {
            id: "unrouted-model".to_string(),
            display_name: String::new(),
            family: "deepseek".to_string(),
            enabled: true,
        })
        .await
        .expect("create unrouted model");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "deepseek-v4-pro".to_string(),
            provider_account_id: deepseek.id.clone(),
            upstream_model_id: "deepseek-v4-pro".to_string(),
            wire_api: "openai-chat".to_string(),
            role: "primary".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create primary route");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "deepseek-v4-pro".to_string(),
            provider_account_id: deepseek.id.clone(),
            upstream_model_id: "deepseek-v4-pro-backup".to_string(),
            wire_api: "openai-chat".to_string(),
            role: "backup".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create duplicate chat route");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "deepseek-v4-pro".to_string(),
            provider_account_id: duplicate.id,
            upstream_model_id: "deepseek-v4-pro".to_string(),
            wire_api: "openai-responses".to_string(),
            role: "backup".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create duplicate route");
        db.create_provider_model_route(CreateProviderModelRouteRequest {
            public_model_id: "gpt-5.5".to_string(),
            provider_account_id: inactive.id,
            upstream_model_id: "gpt-5.5".to_string(),
            wire_api: "openai-responses".to_string(),
            role: "primary".to_string(),
            enabled: true,
            strip_params: Vec::new(),
        })
        .await
        .expect("create inactive route");

        let mut blocked_route_id = None;
        let mut cooling_route_id = None;
        for (id, model_enabled, route_enabled) in [
            ("disabled-model", false, true),
            ("disabled-route", true, false),
            ("blocked-route", true, true),
            ("cooling-route", true, true),
        ] {
            db.create_model_catalog_entry(CreateModelCatalogEntryRequest {
                id: id.to_string(),
                display_name: String::new(),
                family: "deepseek".to_string(),
                enabled: model_enabled,
            })
            .await
            .expect("create ineligible model");
            let route = db
                .create_provider_model_route(CreateProviderModelRouteRequest {
                    public_model_id: id.to_string(),
                    provider_account_id: deepseek.id.clone(),
                    upstream_model_id: id.to_string(),
                    wire_api: "openai-chat".to_string(),
                    role: "primary".to_string(),
                    enabled: route_enabled,
                    strip_params: Vec::new(),
                })
                .await
                .expect("create ineligible route");
            match id {
                "blocked-route" => blocked_route_id = Some(route.id),
                "cooling-route" => cooling_route_id = Some(route.id),
                _ => {}
            }
        }
        db.mark_route_failure(
            blocked_route_id.as_deref().expect("blocked route id"),
            "blocked",
            Some(401),
            "invalid credential",
            None,
        )
        .await
        .expect("block route");
        db.mark_route_failure(
            cooling_route_id.as_deref().expect("cooling route id"),
            "cooling",
            Some(429),
            "rate limited",
            Some(Utc::now() + chrono::Duration::minutes(1)),
        )
        .await
        .expect("cool route");

        let models = db
            .list_routable_model_catalog(&["openai-chat", "openai-responses"])
            .await
            .expect("list model catalog");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "deepseek-v4-pro");
        assert_eq!(models[0].display_name, "DeepSeek V4 Pro");
        assert_eq!(models[0].provider, "deepseek");
        assert_eq!(models[0].wire_api, "openai-chat");

        let models_by_wire = db
            .list_routable_model_catalog_by_wire()
            .await
            .expect("list model catalog by wire");
        assert_eq!(
            models_by_wire
                .iter()
                .map(|model| (model.id.as_str(), model.wire_api.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("deepseek-v4-pro", "openai-chat"),
                ("deepseek-v4-pro", "openai-responses"),
            ]
        );

        let _ = std::fs::remove_file(path);
    }
}
