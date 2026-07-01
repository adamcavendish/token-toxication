use std::{path::Path, sync::Arc};

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    auth::{hash_secret, key_preview},
    models::{
        ApiKeyRecord, ApiKeyView, CreateApiKeyRequest, CreateProviderAccountRequest, Dashboard,
        ProviderAccount, ProviderAccountRecord, RequestLog, UpdateApiKeyRequest,
        UpdateProviderAccountRequest, UsageSummary,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalogEntry {
    pub id: String,
    pub provider: String,
    pub wire_api: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub async fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        }

        let conn = Connection::open(path)?;
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
                model_hint TEXT NOT NULL DEFAULT '',
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
                status_code INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
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
        conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_provider_accounts_wire_api
                ON provider_accounts(wire_api, is_active, status);
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
    ) -> rusqlite::Result<()> {
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
    ) -> rusqlite::Result<Option<String>> {
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

    pub async fn delete_admin_session(&self, token: &str) -> rusqlite::Result<()> {
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
    ) -> rusqlite::Result<ApiKeyView> {
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

    pub async fn list_api_keys(&self) -> rusqlite::Result<Vec<ApiKeyView>> {
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

    pub async fn validate_api_key(&self, secret: &str) -> rusqlite::Result<Option<ApiKeyRecord>> {
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
    ) -> rusqlite::Result<ApiKeyView> {
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

    pub async fn delete_api_key(&self, id: &str) -> rusqlite::Result<bool> {
        let conn = self.conn.lock().await;
        let deleted = conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }

    pub async fn get_api_key(&self, id: &str) -> rusqlite::Result<Option<ApiKeyView>> {
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
    ) -> rusqlite::Result<ProviderAccount> {
        let now = Utc::now();
        let provider = normalize_provider(&input.provider);
        let account = ProviderAccount {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            provider,
            base_url: normalize_base_url(&input.base_url),
            auth_mode: normalize_auth_mode(&input.auth_mode),
            wire_api: normalize_wire_api(&input.wire_api, &input.provider),
            model_hint: input.model_hint,
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
             (id, name, provider, base_url, auth_mode, wire_api, api_key, model_hint, is_active,
              priority, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &account.id,
                &account.name,
                &account.provider,
                &account.base_url,
                &account.auth_mode,
                &account.wire_api,
                input.api_key,
                &account.model_hint,
                bool_to_i64(account.is_active),
                account.priority,
                &account.status,
                account.created_at.to_rfc3339(),
            ],
        )?;
        Ok(account)
    }

    pub async fn list_provider_accounts(&self) -> rusqlite::Result<Vec<ProviderAccount>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, base_url, auth_mode, wire_api, api_key, model_hint, is_active,
                    priority, status, last_error, created_at, last_used_at
             FROM provider_accounts
             ORDER BY priority DESC, created_at DESC",
        )?;
        rows_to_accounts(&mut stmt, params![])
    }

    pub async fn list_model_catalog(
        &self,
        wire_apis: &[&str],
    ) -> rusqlite::Result<Vec<ModelCatalogEntry>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT provider, wire_api, model_hint, created_at
             FROM provider_accounts
             WHERE is_active = 1
               AND status != 'blocked'
               AND model_hint != ''
             ORDER BY priority DESC, created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ModelCatalogEntry {
                provider: row.get(0)?,
                wire_api: row.get(1)?,
                id: row.get::<_, String>(2)?.trim().to_string(),
                created_at: parse_time(row.get::<_, String>(3)?.as_str()),
            })
        })?;
        let mut models = Vec::new();
        for row in rows {
            let model = row?;
            if model.id.is_empty()
                || !wire_apis.contains(&model.wire_api.as_str())
                || models
                    .iter()
                    .any(|existing: &ModelCatalogEntry| existing.id == model.id)
            {
                continue;
            }
            models.push(model);
        }
        Ok(models)
    }

    pub async fn select_provider_account(
        &self,
        model: Option<&str>,
    ) -> rusqlite::Result<Option<ProviderAccountRecord>> {
        self.select_provider_account_for_wire("anthropic-messages", model)
            .await
    }

    pub async fn select_provider_account_for_wire(
        &self,
        wire_api: &str,
        model: Option<&str>,
    ) -> rusqlite::Result<Option<ProviderAccountRecord>> {
        let wire_api = normalize_wire_api(wire_api, "");
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, base_url, auth_mode, wire_api, api_key, model_hint, is_active,
                    priority, status, last_error, created_at, last_used_at
             FROM provider_accounts
             WHERE is_active = 1
               AND status != 'blocked'
               AND wire_api = ?1
               AND (?2 IS NULL OR model_hint = '' OR LOWER(?2) LIKE '%' || LOWER(model_hint) || '%')
             ORDER BY
               CASE
                 WHEN ?2 IS NOT NULL
                  AND model_hint != ''
                  AND LOWER(?2) LIKE '%' || LOWER(model_hint) || '%'
                 THEN 0
                 ELSE 1
               END,
               priority DESC,
               COALESCE(last_used_at, '') ASC,
               created_at ASC
             LIMIT 1",
        )?;
        stmt.query_row(params![wire_api, model], account_from_row)
            .optional()
    }

    pub async fn update_provider_account(
        &self,
        id: &str,
        input: UpdateProviderAccountRequest,
    ) -> rusqlite::Result<ProviderAccount> {
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
        let account = ProviderAccount {
            id: current.id,
            name: input.name.unwrap_or(current.name),
            provider,
            base_url: input
                .base_url
                .map(|url| normalize_base_url(&url))
                .unwrap_or(current.base_url),
            auth_mode: input
                .auth_mode
                .map(|mode| normalize_auth_mode(&mode))
                .unwrap_or(current.auth_mode),
            wire_api,
            model_hint: input.model_hint.unwrap_or(current.model_hint),
            is_active: input.is_active.unwrap_or(current.is_active),
            priority: input.priority.unwrap_or(current.priority),
            status: current.status,
            last_error: current.last_error,
            created_at: current.created_at,
            last_used_at: current.last_used_at,
        };

        let conn = self.conn.lock().await;
        if let Some(api_key) = input.api_key {
            conn.execute(
                "UPDATE provider_accounts SET name = ?1, provider = ?2, base_url = ?3,
                 auth_mode = ?4, wire_api = ?5, api_key = ?6, model_hint = ?7,
                 is_active = ?8, priority = ?9
                 WHERE id = ?10",
                params![
                    &account.name,
                    &account.provider,
                    &account.base_url,
                    &account.auth_mode,
                    &account.wire_api,
                    api_key,
                    &account.model_hint,
                    bool_to_i64(account.is_active),
                    account.priority,
                    &account.id,
                ],
            )?;
        } else {
            conn.execute(
                "UPDATE provider_accounts SET name = ?1, provider = ?2, base_url = ?3,
                 auth_mode = ?4, wire_api = ?5, model_hint = ?6, is_active = ?7, priority = ?8
                 WHERE id = ?9",
                params![
                    &account.name,
                    &account.provider,
                    &account.base_url,
                    &account.auth_mode,
                    &account.wire_api,
                    &account.model_hint,
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
    ) -> rusqlite::Result<Option<ProviderAccount>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, base_url, auth_mode, wire_api, api_key, model_hint, is_active,
                    priority, status, last_error, created_at, last_used_at
             FROM provider_accounts WHERE id = ?1",
        )?;
        stmt.query_row(params![id], account_from_row)
            .optional()
            .map(|record| record.map(|record| record.account))
    }

    pub async fn delete_provider_account(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM provider_accounts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub async fn mark_provider_result(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE provider_accounts SET status = ?1, last_error = ?2, last_used_at = ?3 WHERE id = ?4",
            params![status, error, Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub async fn insert_request_log(&self, log: RequestLog) -> rusqlite::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO request_logs
             (id, api_key_id, provider_account_id, method, path, model, status_code, latency_ms,
              input_tokens, output_tokens, cost_usd, created_at, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                log.id,
                log.api_key_id,
                log.provider_account_id,
                log.method,
                log.path,
                log.model,
                log.status_code,
                log.latency_ms,
                log.input_tokens,
                log.output_tokens,
                log.cost_usd,
                log.created_at.to_rfc3339(),
                log.error,
            ],
        )?;
        Ok(())
    }

    pub async fn list_request_logs(&self, limit: u32) -> rusqlite::Result<Vec<RequestLog>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, api_key_id, provider_account_id, method, path, model, status_code,
                    latency_ms, input_tokens, output_tokens, cost_usd, created_at, error
             FROM request_logs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], request_log_from_row)?;
        rows.collect()
    }

    pub async fn dashboard(&self) -> rusqlite::Result<Dashboard> {
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
        drop(conn);

        Ok(Dashboard {
            active_api_keys,
            total_api_keys,
            healthy_accounts,
            total_accounts,
            usage,
            accounts: self.list_provider_accounts().await?,
            recent_requests: self.list_request_logs(10).await?,
        })
    }
}

fn rows_to_api_keys<P>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
) -> rusqlite::Result<Vec<ApiKeyView>>
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
) -> rusqlite::Result<()> {
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

fn rows_to_accounts<P>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
) -> rusqlite::Result<Vec<ProviderAccount>>
where
    P: rusqlite::Params,
{
    let rows = stmt.query_map(params, account_from_row)?;
    rows.map(|row| row.map(|record| record.account)).collect()
}

fn api_key_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApiKeyRecord> {
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

fn account_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderAccountRecord> {
    Ok(ProviderAccountRecord {
        account: ProviderAccount {
            id: row.get(0)?,
            name: row.get(1)?,
            provider: row.get(2)?,
            base_url: row.get(3)?,
            auth_mode: row.get(4)?,
            wire_api: row.get(5)?,
            model_hint: row.get(7)?,
            is_active: row.get::<_, i64>(8)? == 1,
            priority: row.get(9)?,
            status: row.get(10)?,
            last_error: row.get(11)?,
            created_at: parse_time(row.get::<_, String>(12)?.as_str()),
            last_used_at: parse_time_opt(row.get::<_, Option<String>>(13)?.as_deref()),
        },
        api_key: row.get(6)?,
    })
}

fn request_log_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestLog> {
    Ok(RequestLog {
        id: row.get(0)?,
        api_key_id: row.get(1)?,
        provider_account_id: row.get(2)?,
        method: row.get(3)?,
        path: row.get(4)?,
        model: row.get(5)?,
        status_code: row.get::<_, i64>(6)? as u16,
        latency_ms: row.get::<_, i64>(7)? as u64,
        input_tokens: row.get::<_, i64>(8)? as u64,
        output_tokens: row.get::<_, i64>(9)? as u64,
        cost_usd: row.get(10)?,
        created_at: parse_time(row.get::<_, String>(11)?.as_str()),
        error: row.get(12)?,
    })
}

fn count(conn: &Connection, sql: &str) -> rusqlite::Result<u64> {
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .map(|value| value as u64)
}

fn usage_summary(conn: &Connection, today: NaiveDate) -> rusqlite::Result<UsageSummary> {
    let prefix = today.to_string();
    let (requests_today, tokens_today, estimated_cost_today) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(input_tokens + output_tokens), 0), COALESCE(SUM(cost_usd), 0)
         FROM request_logs
         WHERE created_at LIKE ?1 || '%'",
        params![prefix],
        |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, f64>(2)?,
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
        total_requests,
        total_tokens,
        estimated_cost_today,
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
    let trimmed = value.trim().to_lowercase();
    match trimmed.as_str() {
        "" | "claude" => "anthropic".to_string(),
        "codex" => "openai".to_string(),
        "deepseek-v4" | "deepseek-v4-flash" | "deepseek-v4-pro" => "deepseek".to_string(),
        "dashscope" | "aliyun" | "qwen3" => "qwen".to_string(),
        "moonshot" => "kimi".to_string(),
        "zhipu" | "zhipuai" => "glm".to_string(),
        _ => trimmed,
    }
}

fn normalize_auth_mode(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "bearer" | "authorization" => "bearer".to_string(),
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
        "" => match normalize_provider(provider).as_str() {
            "openai" => "openai-responses".to_string(),
            "deepseek" | "glm" | "kimi" | "qwen" => "openai-chat".to_string(),
            _ => "anthropic-messages".to_string(),
        },
        _ => "anthropic-messages".to_string(),
    }
}

fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn model_specific_provider_wins_over_catch_all() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");

        db.create_provider_account(CreateProviderAccountRequest {
            name: "Catch all".to_string(),
            provider: "openai-compatible".to_string(),
            base_url: "https://catch-all.example.com/".to_string(),
            auth_mode: "bearer".to_string(),
            wire_api: "openai-chat".to_string(),
            api_key: "catch-all-key".to_string(),
            model_hint: String::new(),
            is_active: true,
            priority: 100,
        })
        .await
        .expect("create catch-all account");
        db.create_provider_account(CreateProviderAccountRequest {
            name: "Qwen".to_string(),
            provider: "qwen".to_string(),
            base_url: "https://qwen.example.com/".to_string(),
            auth_mode: "bearer".to_string(),
            wire_api: "openai-chat".to_string(),
            api_key: "qwen-key".to_string(),
            model_hint: "qwen".to_string(),
            is_active: true,
            priority: 0,
        })
        .await
        .expect("create qwen account");

        let selected = db
            .select_provider_account_for_wire("openai-chat", Some("qwen-plus"))
            .await
            .expect("select provider")
            .expect("selected provider");

        assert_eq!(selected.account.provider, "qwen");
        assert_eq!(selected.account.base_url, "https://qwen.example.com");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn openai_compatible_provider_aliases_default_to_chat() {
        for provider in [
            "deepseek",
            "qwen",
            "dashscope",
            "kimi",
            "moonshot",
            "glm",
            "zhipu",
        ] {
            assert_eq!(normalize_wire_api("", provider), "openai-chat");
        }
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
    async fn model_catalog_lists_active_model_hints_for_wire_api() {
        let path =
            std::env::temp_dir().join(format!("token-toxication-{}.sqlite3", Uuid::new_v4()));
        let db = Db::open(&path).await.expect("open test database");

        db.create_provider_account(CreateProviderAccountRequest {
            name: "DeepSeek".to_string(),
            provider: "deepseek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            auth_mode: "bearer".to_string(),
            wire_api: "openai-chat".to_string(),
            api_key: "deepseek-key".to_string(),
            model_hint: "deepseek-v4-pro".to_string(),
            is_active: true,
            priority: 10,
        })
        .await
        .expect("create deepseek account");
        db.create_provider_account(CreateProviderAccountRequest {
            name: "Catch all".to_string(),
            provider: "openai-compatible".to_string(),
            base_url: "https://catch-all.example.com".to_string(),
            auth_mode: "bearer".to_string(),
            wire_api: "openai-chat".to_string(),
            api_key: "catch-all-key".to_string(),
            model_hint: String::new(),
            is_active: true,
            priority: 0,
        })
        .await
        .expect("create catch-all account");
        db.create_provider_account(CreateProviderAccountRequest {
            name: "Inactive".to_string(),
            provider: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            auth_mode: "bearer".to_string(),
            wire_api: "openai-responses".to_string(),
            api_key: "openai-key".to_string(),
            model_hint: "gpt-5".to_string(),
            is_active: false,
            priority: 100,
        })
        .await
        .expect("create inactive account");
        db.create_provider_account(CreateProviderAccountRequest {
            name: "Duplicate".to_string(),
            provider: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            auth_mode: "bearer".to_string(),
            wire_api: "openai-responses".to_string(),
            api_key: "openai-key".to_string(),
            model_hint: "deepseek-v4-pro".to_string(),
            is_active: true,
            priority: 0,
        })
        .await
        .expect("create duplicate account");

        let models = db
            .list_model_catalog(&["openai-chat", "openai-responses"])
            .await
            .expect("list model catalog");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "deepseek-v4-pro");
        assert_eq!(models[0].provider, "deepseek");
        assert_eq!(models[0].wire_api, "openai-chat");

        let _ = std::fs::remove_file(path);
    }
}
