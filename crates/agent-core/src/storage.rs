use crate::{error::Result, model::*};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;

#[derive(Clone)]
pub struct Storage {
    pub(crate) pool: SqlitePool,
}

impl Storage {
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        // Processes do not survive a core restart; never silently replay a turn.
        sqlx::query("UPDATE sessions SET status = 'interrupted' WHERE status = 'running'")
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }

    pub async fn workspaces(&self) -> Result<Vec<Workspace>> {
        Ok(
            sqlx::query_as("SELECT * FROM workspaces ORDER BY created_at, id")
                .fetch_all(&self.pool)
                .await?,
        )
    }
    pub async fn workspace(&self, id: &str) -> Result<Workspace> {
        sqlx::query_as("SELECT * FROM workspaces WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(crate::CoreError::NotFound)
    }
    pub async fn sessions(&self) -> Result<Vec<Session>> {
        Ok(
            sqlx::query_as("SELECT * FROM sessions ORDER BY updated_at DESC, id")
                .fetch_all(&self.pool)
                .await?,
        )
    }
    pub async fn session(&self, id: &str) -> Result<Session> {
        sqlx::query_as("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(crate::CoreError::NotFound)
    }
    pub async fn accounts(&self) -> Result<Vec<AccountProfile>> {
        Ok(
            sqlx::query_as("SELECT * FROM account_profiles ORDER BY created_at, id")
                .fetch_all(&self.pool)
                .await?,
        )
    }
    pub async fn settings(&self) -> Result<Settings> {
        let value: String =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = 'check_updates_on_start'")
                .fetch_one(&self.pool)
                .await?;
        Ok(Settings {
            check_updates_on_start: value == "true",
        })
    }
    pub async fn save_settings(&self, settings: Settings) -> Result<()> {
        sqlx::query("UPDATE settings SET value = ? WHERE key = 'check_updates_on_start'")
            .bind(settings.check_updates_on_start.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn set_provider_session(&self, session: &str, provider_session: &str) -> Result<()> {
        sqlx::query("UPDATE sessions SET provider_session_id = ? WHERE id = ?")
            .bind(provider_session)
            .bind(session)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn append(
        &self,
        session: &str,
        run: &str,
        payload: EventPayload,
        status: Option<&str>,
    ) -> Result<Event> {
        let timestamp = now();
        let mut tx = self.pool.begin().await?;
        let sequence = sqlx::query(
            "INSERT INTO events(session_id, run_id, timestamp, payload) VALUES (?, ?, ?, ?)",
        )
        .bind(session)
        .bind(run)
        .bind(timestamp)
        .bind(serde_json::to_string(&payload)?)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();
        if let Some(status) = status {
            sqlx::query("UPDATE sessions SET status = ?, updated_at = ? WHERE id = ?")
                .bind(status)
                .bind(timestamp)
                .bind(session)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Event {
            sequence,
            session_id: session.into(),
            run_id: run.into(),
            timestamp,
            payload,
        })
    }
    pub async fn events(&self, session: &str, before: Option<i64>) -> Result<Vec<Event>> {
        self.session(session).await?;
        let rows = sqlx::query("SELECT * FROM events WHERE session_id = ? AND sequence < ? ORDER BY sequence DESC LIMIT 300")
            .bind(session).bind(before.unwrap_or(i64::MAX)).fetch_all(&self.pool).await?;
        let mut events = Vec::with_capacity(rows.len());
        for row in rows.into_iter().rev() {
            events.push(Event {
                sequence: row.get("sequence"),
                session_id: row.get("session_id"),
                run_id: row.get("run_id"),
                timestamp: row.get("timestamp"),
                payload: serde_json::from_str(row.get("payload"))?,
            });
        }
        Ok(events)
    }
}
