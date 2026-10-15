//! Compatibility with the CRLF copies of migrations 6/7 shipped in local Windows builds.
//! SQL and persisted history stay immutable: only the in-memory expected checksum changes
//! for an exact known equivalent. All other SQLx integrity checks remain enabled.
use crate::Result;
use sha2::{Digest, Sha384};
use sqlx::{migrate::Migration, SqlitePool};

fn canonical(migration: &Migration) -> Option<Migration> {
    let expected = match migration.version {
        6 => "20425b8b69697186aa254c207b92dd2d8ef4effaa575c2fe5730a6a70920bf0914bb36f4e924e466d044ee1ed0fe0bc6",
        7 => "a9443f08318c164d93d19be4d0be234edd42b29ba0efebdc957d917af8d4d40cad4541c7e5e00a2b4a366a85fe299f20",
        _ => return None,
    };
    let sql = migration.sql.replace("\r\n", "\n");
    if format!("{:x}", Sha384::digest(sql.as_bytes())) != expected {
        return None;
    }
    Some(Migration::new(
        migration.version,
        migration.description.clone(),
        migration.migration_type,
        sql.into(),
        migration.no_tx,
    ))
}

pub(super) async fn run(pool: &SqlitePool) -> Result<()> {
    let mut migrator = sqlx::migrate!("./migrations");
    let has_history: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    let applied: Vec<(i64, Vec<u8>)> = if has_history {
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?
    } else {
        vec![]
    };
    for migration in migrator.migrations.to_mut() {
        let known_windows_variant = canonical(migration);
        let allow_legacy = known_windows_variant.is_some();
        // New local builds use the same SQL bytes/checksum as a clean Git checkout,
        // including for future untracked migrations created by Windows editors.
        let mut normalized = known_windows_variant.unwrap_or_else(|| {
            Migration::new(
                migration.version,
                migration.description.clone(),
                migration.migration_type,
                migration.sql.replace("\r\n", "\n").into(),
                migration.no_tx,
            )
        });
        if allow_legacy {
            let crlf = Sha384::digest(normalized.sql.replace('\n', "\r\n").as_bytes());
            if let Some((_, checksum)) = applied.iter().find(|(v, _)| *v == migration.version) {
                if checksum.as_slice() == &crlf[..] {
                    normalized.checksum = checksum.clone().into();
                }
            }
        }
        *migration = normalized;
    }
    migrator.run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{storage::Storage, test_support::TestDirectory, CoreError};
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn old_windows_and_release_databases_keep_history_and_chat_data() {
        for legacy in [false, true] {
            let temp = TestDirectory::new().unwrap();
            let path = temp.path().join("workspace.db");
            let storage = Storage::open(&path).await.unwrap();
            sqlx::query("INSERT INTO workspaces VALUES ('project','Fixture','fixture',0)")
                .execute(&storage.pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO sessions (id,workspace_id,provider,account_profile_id,model,title,status,permission_profile,working_directory,created_at,updated_at,parent_session_id,fast_mode) VALUES ('chat','project','mock','mock-local','mock','Сохранённый чат','idle','workspace_auto','fixture',0,0,'chat',1)")
                .execute(&storage.pool).await.unwrap();
            sqlx::query("INSERT INTO events (session_id,run_id,timestamp,payload) VALUES ('chat','run',0,'{\"type\":\"assistant_text_delta\",\"text\":\"История\"}')")
                .execute(&storage.pool).await.unwrap();
            for migration in sqlx::migrate!("./migrations").iter() {
                if let Some(normalized) = canonical(migration) {
                    let bytes = if legacy {
                        Sha384::digest(normalized.sql.replace('\n', "\r\n").as_bytes()).to_vec()
                    } else {
                        normalized.checksum.to_vec()
                    };
                    sqlx::query("UPDATE _sqlx_migrations SET checksum=? WHERE version=?")
                        .bind(bytes)
                        .bind(migration.version)
                        .execute(&storage.pool)
                        .await
                        .unwrap();
                }
            }
            let before: Vec<(i64, Vec<u8>)> =
                sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
                    .fetch_all(&storage.pool)
                    .await
                    .unwrap();
            storage.pool.close().await;
            for _ in 0..2 {
                let reopened = Storage::open(&path).await.unwrap();
                let after: Vec<(i64, Vec<u8>)> = sqlx::query_as(
                    "SELECT version,checksum FROM _sqlx_migrations ORDER BY version",
                )
                .fetch_all(&reopened.pool)
                .await
                .unwrap();
                assert_eq!(before, after);
                let session = reopened.session("chat").await.unwrap();
                assert_eq!(session.permission_profile, "workspace_auto");
                assert!(session.fast_mode); // Migration 6 must not run again.
                assert_eq!(session.title, "Сохранённый чат");
                assert_eq!(reopened.accounts().await.unwrap().len(), 1);
                let count: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE session_id='chat'")
                        .fetch_one(&reopened.pool)
                        .await
                        .unwrap();
                assert_eq!(count, 1);
                reopened.pool.close().await;
            }
        }
    }

    #[tokio::test]
    async fn pending_migration_applies_after_legacy_history() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut old = sqlx::migrate!("./migrations");
        old.migrations = old
            .iter()
            .filter(|m| m.version < 7)
            .map(|m| {
                let sql = m.sql.replace("\r\n", "\n");
                Migration::new(
                    m.version,
                    m.description.clone(),
                    m.migration_type,
                    if m.version == 6 {
                        sql.replace('\n', "\r\n")
                    } else {
                        sql
                    }
                    .into(),
                    m.no_tx,
                )
            })
            .collect::<Vec<_>>()
            .into();
        old.run(&pool).await.unwrap();
        let before: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=6")
                .fetch_one(&pool)
                .await
                .unwrap();
        run(&pool).await.unwrap();
        let after: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=6")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, after);
        sqlx::query("SELECT conversation_settings FROM sessions")
            .execute(&pool)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM _sqlx_migrations WHERE version=7 AND success=1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn unknown_checksum_and_failed_migrations_remain_errors() {
        for (version, checksum, success) in [
            (6, Some(vec![7u8; 48]), true),
            (1, Some(vec![7u8; 48]), true),
            (7, None, false),
        ] {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            run(&pool).await.unwrap();
            if let Some(checksum) = checksum {
                sqlx::query("UPDATE _sqlx_migrations SET checksum=? WHERE version=?")
                    .bind(checksum)
                    .bind(version)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            sqlx::query("UPDATE _sqlx_migrations SET success=? WHERE version=?")
                .bind(success)
                .bind(version)
                .execute(&pool)
                .await
                .unwrap();
            assert!(matches!(run(&pool).await, Err(CoreError::Migration(_))));
        }
    }

    #[test]
    fn compatibility_is_limited_to_exact_historical_sql() {
        for migration in sqlx::migrate!("./migrations").iter() {
            let mut changed = migration.clone();
            changed.sql = format!("{}\n-- changed", changed.sql).into();
            assert!(canonical(&changed).is_none());
            assert_eq!(
                canonical(migration).is_some(),
                [6, 7].contains(&migration.version)
            );
        }
    }
}
