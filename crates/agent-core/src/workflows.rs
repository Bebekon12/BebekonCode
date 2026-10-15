//! Local task queue, isolated branches and file checkpoints; no inference for management.
use crate::{model::*, Core, CoreError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) struct ProjectLease {
    root: PathBuf,
    locks: Arc<Mutex<HashMap<PathBuf, String>>>,
}
impl Drop for ProjectLease {
    fn drop(&mut self) {
        if let Ok(mut locks) = self.locks.lock() {
            locks.remove(&self.root);
        }
    }
}

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QueuedTask {
    pub id: String,
    pub session_id: String,
    pub prompt: String,
    pub attachments: String,
    pub state: String,
    pub run_id: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
}
#[derive(Serialize, Deserialize)]
struct QueuedFile {
    file: crate::attachments::AttachedFile,
    digest: String,
}
fn queued_digest(session: &Session, file: &crate::attachments::AttachedFile) -> Result<String> {
    use std::io::Read;
    let root = Path::new(&session.working_directory).canonicalize()?;
    let recorded = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\"));
    let relative = Path::new(&file.path)
        .strip_prefix(&recorded)
        .map_err(|_| invalid("Вложение вне чата"))?;
    let parts: Vec<_> = relative.components().collect();
    if parts.len() != 2 || parts[0].as_os_str() != ".bebekon-attachments" {
        return Err(invalid("Недопустимый путь вложения"));
    }
    let path = crate::files::resolve(&root, &relative.to_string_lossy().replace('\\', "/"), false)?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(20 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err(invalid("Вложение слишком большое"));
    }
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub root: String,
    pub label: String,
    pub created_at: i64,
    pub file_count: i64,
    pub limited: bool,
}
#[derive(Serialize)]
pub struct CheckpointDiff {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub expected: String,
}
#[derive(Serialize, Deserialize)]
struct Archive {
    files: BTreeMap<String, String>,
}

pub(crate) fn configuration(s: &Session) -> String {
    serde_json::json!([
        s.provider,
        s.account_profile_id,
        s.model,
        s.reasoning_effort,
        s.fast_mode,
        s.conversation_settings,
        s.permission_profile,
        s.tool_policy,
        s.working_directory,
        s.chat_mode,
        s.role
    ])
    .to_string()
}
fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
fn fingerprint(text: Option<&str>) -> String {
    text.map(|t| format!("{:x}", Sha256::digest(t.as_bytes())))
        .unwrap_or_else(|| "missing".into())
}
fn eligible(path: &str) -> bool {
    !crate::changes::excluded(path)
        && !path.split('/').any(|p| {
            matches!(
                p,
                "node_modules"
                    | "target"
                    | ".bebekon-attachments"
                    | "providers"
                    | "checkpoints"
                    | "worktrees"
            )
        })
}
async fn paths(root: &Path) -> Result<Vec<String>> {
    let data = crate::git::read_git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )
    .await?;
    let paths: BTreeSet<String> = data
        .split(|v| *v == 0)
        .filter(|v| !v.is_empty())
        .map(|v| String::from_utf8(v.to_vec()).map_err(|_| invalid("Имя файла не UTF-8")))
        .collect::<Result<_>>()?;
    if paths.len() > 20_000 {
        return Err(invalid(
            "В проекте больше 20 000 файлов; операция недоступна",
        ));
    }
    Ok(paths.into_iter().filter(|p| eligible(p)).collect())
}
fn read_text(root: &Path, path: &str) -> Result<Option<String>> {
    let resolved = crate::files::resolve(root, path, true)?;
    if !resolved.exists() {
        return Ok(None);
    }
    if !resolved.is_file() || std::fs::metadata(&resolved)?.len() > 256 * 1024 {
        return Err(invalid("Файл не является небольшим текстовым документом"));
    }
    let text = std::fs::read_to_string(resolved)?;
    if text.contains('\0') || crate::redaction::redact(&text) != text {
        return Err(invalid(
            "Файл содержит защищённые данные или двоичное содержимое",
        ));
    }
    Ok(Some(text))
}

impl Core {
    pub(crate) async fn queue_configuration(&self, session: &Session) -> Result<String> {
        let members: Vec<Session> = sqlx::query_as(
            "SELECT * FROM sessions WHERE parent_session_id=? AND chat_mode!='task' ORDER BY id",
        )
        .bind(&session.id)
        .fetch_all(&self.storage.pool)
        .await?;
        let root = configuration(session);
        if members.is_empty() {
            Ok(root)
        } else {
            Ok(
                serde_json::json!([root, members.iter().map(configuration).collect::<Vec<_>>()])
                    .to_string(),
            )
        }
    }
    pub async fn project_file_rules(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::claude::rules::FileRule>> {
        let s = self.storage.session(session_id).await?;
        if s.provider != "anthropic" {
            return Err(CoreError::ProviderUnavailable);
        }
        crate::claude::rules::read(&self.account(&s.account_profile_id).await?, &s)
    }
    pub async fn save_project_file_rules(
        &self,
        session_id: &str,
        rules: Vec<crate::claude::rules::FileRule>,
    ) -> Result<()> {
        let _lease = self.lease(session_id, CancellationToken::new())?;
        let s = self.storage.session(session_id).await?;
        crate::claude::rules::save(&self.account(&s.account_profile_id).await?, &s, rules)
    }
    pub(crate) fn project_lease(&self, session: &Session) -> Result<Option<ProjectLease>> {
        if session.permission_profile == "read_only" && session.chat_mode == "single" {
            return Ok(None);
        }
        let root = Path::new(&session.working_directory).canonicalize()?;
        let mut locks = self.project_locks.lock().map_err(|_| CoreError::Busy)?;
        // Nested folders can touch the same files, so treat them as one writing area too.
        if locks
            .keys()
            .any(|p| root.starts_with(p) || p.starts_with(&root))
        {
            return Err(CoreError::Busy);
        }
        locks.insert(root.clone(), session.id.clone());
        Ok(Some(ProjectLease {
            root,
            locks: self.project_locks.clone(),
        }))
    }
    pub(crate) fn start_queue_worker(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let wake = self.queue_wake.clone();
        let stop = self.workflow_stop.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! { _=stop.cancelled()=>break, _=wake.notified()=>{} }
                let Some(core) = weak.upgrade() else { break };
                if core.drain_queue().await.is_err() {
                    let _=sqlx::query("UPDATE queued_tasks SET state='paused',error='Очередь остановлена из-за ошибки хранилища' WHERE state IN ('running','queued')").execute(&core.storage.pool).await;
                }
            }
        });
    }
    pub async fn task_queue(&self) -> Result<Vec<QueuedTask>> {
        Ok(sqlx::query_as("SELECT id,session_id,prompt,attachments,state,run_id,error,created_at FROM queued_tasks ORDER BY CASE WHEN state IN ('queued','running','paused') THEN 0 ELSE 1 END,position,created_at,rowid LIMIT 500").fetch_all(&self.storage.pool).await?)
    }
    pub async fn enqueue_task(
        self: &Arc<Self>,
        session_id: &str,
        prompt: String,
    ) -> Result<String> {
        self.enqueue_message(session_id, prompt, vec![]).await
    }
    pub async fn enqueue_message(
        self: &Arc<Self>,
        session_id: &str,
        prompt: String,
        attachments: Vec<crate::attachments::Attachment>,
    ) -> Result<String> {
        let _operation = self.queue_ops.lock().await;
        if prompt.trim().is_empty() || crate::redaction::redact(&prompt) != prompt {
            return Err(invalid("Пустое сообщение или защищённые данные"));
        }
        let session = self.storage.session(session_id).await?;
        if session.parent_session_id.is_some() {
            return Err(invalid("Очередь доступна в основном чате"));
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM queued_tasks WHERE state IN ('queued','paused','running')",
        )
        .fetch_one(&self.storage.pool)
        .await?;
        if count >= 100 {
            return Err(invalid("В очереди уже 100 задач"));
        }
        let id = Uuid::new_v4().to_string();
        let attachment_session = session.clone();
        let saved = tokio::task::spawn_blocking(move || -> Result<Vec<QueuedFile>> {
            crate::attachments::prepare(&attachment_session, attachments)?
                .into_iter()
                .map(|file| {
                    let digest = queued_digest(&attachment_session, &file)?;
                    Ok(QueuedFile { file, digest })
                })
                .collect()
        })
        .await
        .map_err(|_| CoreError::Busy)??;
        sqlx::query("INSERT INTO queued_tasks(id,session_id,prompt,configuration,state,created_at,attachments,position) VALUES (?,?,?,?,'queued',?,?,(SELECT COALESCE(MAX(position),0)+1 FROM queued_tasks))")
            .bind(&id).bind(session_id).bind(prompt).bind(self.queue_configuration(&session).await?).bind(now()).bind(serde_json::to_string(&saved)?).execute(&self.storage.pool).await?;
        self.queue_wake.notify_one();
        Ok(id)
    }
    pub async fn change_queued_task(self: &Arc<Self>, id: &str, resume: bool) -> Result<()> {
        let _operation = self.queue_ops.lock().await;
        let task:QueuedTask=sqlx::query_as("SELECT id,session_id,prompt,attachments,state,run_id,error,created_at FROM queued_tasks WHERE id=?").bind(id).fetch_optional(&self.storage.pool).await?.ok_or(CoreError::NotFound)?;
        if resume {
            if task.state != "paused" {
                return Err(invalid("Продолжить можно только приостановленную задачу"));
            }
            let session = self.storage.session(&task.session_id).await?;
            sqlx::query("UPDATE queued_tasks SET state='queued',configuration=?,error=NULL WHERE id=? AND state='paused'").bind(self.queue_configuration(&session).await?).bind(id).execute(&self.storage.pool).await?;
            self.queue_wake.notify_one();
        } else {
            sqlx::query("UPDATE queued_tasks SET state='cancelled' WHERE id=? AND state IN ('queued','paused','running','failed')").bind(id).execute(&self.storage.pool).await?;
            if let Some(token) = self
                .queue_tokens
                .lock()
                .map_err(|_| CoreError::Busy)?
                .get(id)
            {
                token.cancel();
            }
        }
        Ok(())
    }
    pub async fn edit_queued_task(&self, id: &str, prompt: String, direction: i32) -> Result<()> {
        let _operation = self.queue_ops.lock().await;
        if prompt.trim().is_empty()
            || crate::redaction::redact(&prompt) != prompt
            || !(-1..=1).contains(&direction)
        {
            return Err(invalid("Недопустимое изменение очереди"));
        }
        let task:Option<(String,i64)>=sqlx::query_as("SELECT session_id,position FROM queued_tasks WHERE id=? AND state IN ('queued','paused')")
            .bind(id).fetch_optional(&self.storage.pool).await?;
        let (session, position) =
            task.ok_or_else(|| invalid("Задание уже выполняется или завершено"))?;
        let mut tx = self.storage.pool.begin().await?;
        sqlx::query("UPDATE queued_tasks SET prompt=? WHERE id=?")
            .bind(prompt)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if direction != 0 {
            let query = if direction < 0 {
                "SELECT id,position FROM queued_tasks WHERE session_id=? AND state IN ('queued','paused') AND position<? ORDER BY position DESC LIMIT 1"
            } else {
                "SELECT id,position FROM queued_tasks WHERE session_id=? AND state IN ('queued','paused') AND position>? ORDER BY position LIMIT 1"
            };
            let neighbour: Option<(String, i64)> = sqlx::query_as(query)
                .bind(session)
                .bind(position)
                .fetch_optional(&mut *tx)
                .await?;
            if let Some((other, other_position)) = neighbour {
                sqlx::query("UPDATE queued_tasks SET position=? WHERE id=?")
                    .bind(other_position)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE queued_tasks SET position=? WHERE id=?")
                    .bind(position)
                    .bind(other)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        self.queue_wake.notify_one();
        Ok(())
    }
    async fn drain_queue(self: &Arc<Self>) -> Result<()> {
        loop {
            if self.workflow_stop.is_cancelled() {
                return Ok(());
            }
            let operation = self.queue_ops.lock().await;
            let next:Option<(String,String,String,String)>=sqlx::query_as("SELECT id,session_id,prompt,configuration FROM queued_tasks WHERE state='queued' ORDER BY position,created_at,rowid LIMIT 1").fetch_optional(&self.storage.pool).await?;
            let Some((id, session_id, prompt, config)) = next else {
                return Ok(());
            };
            let session = self.storage.session(&session_id).await?;
            if self.queue_configuration(&session).await? != config {
                sqlx::query("UPDATE queued_tasks SET state='paused',error='Настройки чата изменились; подтвердите продолжение' WHERE id=?").bind(id).execute(&self.storage.pool).await?;
                continue;
            }
            let claimed = sqlx::query(
                "UPDATE queued_tasks SET state='running' WHERE id=? AND state='queued'",
            )
            .bind(&id)
            .execute(&self.storage.pool)
            .await?
            .rows_affected();
            if claimed == 0 {
                continue;
            }
            let files: String =
                sqlx::query_scalar("SELECT attachments FROM queued_tasks WHERE id=?")
                    .bind(&id)
                    .fetch_one(&self.storage.pool)
                    .await?;
            let files: Vec<QueuedFile> = serde_json::from_str(&files)?;
            let attachment_session = session.clone();
            let validated = tokio::task::spawn_blocking(
                move || -> Result<Vec<crate::attachments::AttachedFile>> {
                    files
                        .into_iter()
                        .map(|saved| {
                            if queued_digest(&attachment_session, &saved.file)? != saved.digest {
                                return Err(invalid(
                                    "Вложение очереди изменилось; прикрепите его заново",
                                ));
                            }
                            Ok(saved.file)
                        })
                        .collect()
                },
            )
            .await
            .map_err(|_| CoreError::Busy)?;
            let prepared_files = match validated {
                Ok(files) => files,
                Err(error) => {
                    sqlx::query("UPDATE queued_tasks SET state='failed',error=? WHERE id=?")
                        .bind(crate::redaction::redact(&error.to_string()))
                        .bind(&id)
                        .execute(&self.storage.pool)
                        .await?;
                    self.pause_queue().await?;
                    continue;
                }
            };
            let task_cancel = CancellationToken::new();
            let run = match self
                .send_message_checked(
                    &session_id,
                    prompt,
                    vec![],
                    Some(&config),
                    Some(task_cancel.clone()),
                    Some(prepared_files),
                )
                .await
            {
                Ok(run) => run,
                Err(CoreError::Busy) => {
                    sqlx::query(
                        "UPDATE queued_tasks SET state='queued' WHERE id=? AND state='running'",
                    )
                    .bind(id)
                    .execute(&self.storage.pool)
                    .await?;
                    drop(operation);
                    tokio::select! {_=self.workflow_stop.cancelled()=>return Ok(()),_=tokio::time::sleep(Duration::from_millis(200))=>{}}
                    continue;
                }
                Err(error) => {
                    sqlx::query("UPDATE queued_tasks SET state='failed',error=? WHERE id=? AND state='running'").bind(crate::redaction::redact(&error.to_string())).bind(id).execute(&self.storage.pool).await?;
                    self.pause_queue().await?;
                    return Ok(());
                }
            };
            self.queue_tokens
                .lock()
                .map_err(|_| CoreError::Busy)?
                .insert(id.clone(), task_cancel);
            sqlx::query("UPDATE queued_tasks SET run_id=? WHERE id=?")
                .bind(&run)
                .bind(&id)
                .execute(&self.storage.pool)
                .await?;
            drop(operation);
            let status = loop {
                if self.workflow_stop.is_cancelled() {
                    return Ok(());
                }
                let terminal:Option<String>=sqlx::query_scalar("SELECT json_extract(payload,'$.type') FROM events WHERE session_id=? AND run_id=? AND json_extract(payload,'$.type') IN ('turn_completed','session_stopped','provider_error') ORDER BY sequence DESC LIMIT 1")
                    .bind(&session_id).bind(&run).fetch_optional(&self.storage.pool).await?;
                if let Some(terminal) = terminal {
                    break if terminal == "turn_completed" {
                        "completed"
                    } else {
                        "failed"
                    };
                }
                tokio::select! {_=self.workflow_stop.cancelled()=>return Ok(()),_=tokio::time::sleep(Duration::from_millis(100))=>{}}
            };
            self.queue_tokens
                .lock()
                .map_err(|_| CoreError::Busy)?
                .remove(&id);
            sqlx::query("UPDATE queued_tasks SET state=?,error=? WHERE id=? AND state='running'")
                .bind(if status == "completed" {
                    "completed"
                } else {
                    "failed"
                })
                .bind(if status == "completed" {
                    None
                } else {
                    Some("Запуск не завершён; проверьте историю чата")
                })
                .bind(id)
                .execute(&self.storage.pool)
                .await?;
            if status != "completed" {
                self.pause_queue().await?;
                self.emit(&session_id, &run, EventPayload::QueueChanged, None)
                    .await?;
                return Ok(());
            }
            self.emit(&session_id, &run, EventPayload::QueueChanged, None)
                .await?;
        }
    }
    async fn pause_queue(&self) -> Result<()> {
        sqlx::query("UPDATE queued_tasks SET state='paused',error='Предыдущая задача остановлена или завершилась ошибкой; автоматического повтора нет' WHERE state='queued'").execute(&self.storage.pool).await?;
        Ok(())
    }
    pub async fn checkpoints(&self, session_id: &str) -> Result<Vec<Checkpoint>> {
        self.storage.session(session_id).await?;
        Ok(sqlx::query_as("SELECT * FROM checkpoints WHERE session_id=? ORDER BY created_at DESC,rowid DESC LIMIT 100").bind(session_id).fetch_all(&self.storage.pool).await?)
    }
    pub async fn create_checkpoint(&self, session_id: &str, label: &str) -> Result<Checkpoint> {
        let _lease = self.lease(session_id, CancellationToken::new())?;
        let mut session = self.storage.session(session_id).await?;
        session.permission_profile = "standard".into();
        let _project = self.project_lease(&session)?;
        self.capture_checkpoint(&session, label).await
    }
    pub(crate) async fn capture_checkpoint(
        &self,
        session: &Session,
        label: &str,
    ) -> Result<Checkpoint> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints WHERE session_id=?")
            .bind(&session.id)
            .fetch_one(&self.storage.pool)
            .await?;
        if count >= 100 {
            return Err(invalid(
                "Сохранено 100 контрольных точек; удалите ненужные в «Задачи и версии»",
            ));
        }
        let label: String = crate::redaction::redact(label).chars().take(120).collect();
        let root = Path::new(&session.working_directory);
        let mut files = BTreeMap::new();
        let mut total = 0;
        let mut limited = false;
        for path in paths(root).await? {
            match read_text(root, &path) {
                Ok(Some(text)) if total + text.len() <= 16 * 1024 * 1024 => {
                    total += text.len();
                    files.insert(path, text);
                }
                Ok(None) => {}
                _ => limited = true,
            }
        }
        let id = Uuid::new_v4().to_string();
        let checkpoint = Checkpoint {
            id: id.clone(),
            session_id: session.id.clone(),
            root: session.working_directory.clone(),
            label,
            created_at: now(),
            file_count: files.len() as i64,
            limited,
        };
        let dir = self.data_dir.join("checkpoints");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec(&Archive { files })?,
        )?;
        sqlx::query("INSERT INTO checkpoints VALUES (?,?,?,?,?,?,?)")
            .bind(&id)
            .bind(&checkpoint.session_id)
            .bind(&checkpoint.root)
            .bind(&checkpoint.label)
            .bind(checkpoint.created_at)
            .bind(checkpoint.file_count)
            .bind(limited)
            .execute(&self.storage.pool)
            .await?;
        Ok(checkpoint)
    }
    async fn archive(&self, session_id: &str, id: &str) -> Result<(Session, Archive)> {
        Uuid::parse_str(id).map_err(|_| CoreError::NotFound)?;
        let session = self.storage.session(session_id).await?;
        let checkpoint: Checkpoint =
            sqlx::query_as("SELECT * FROM checkpoints WHERE id=? AND session_id=?")
                .bind(id)
                .bind(session_id)
                .fetch_optional(&self.storage.pool)
                .await?
                .ok_or(CoreError::NotFound)?;
        if session.working_directory != checkpoint.root {
            return Err(invalid("Контрольная точка относится к другой папке"));
        }
        let file = self.data_dir.join("checkpoints").join(format!("{id}.json"));
        if std::fs::metadata(&file)?.len() > 32 * 1024 * 1024 {
            return Err(invalid("Контрольная точка слишком велика"));
        }
        let archive: Archive = serde_json::from_slice(&std::fs::read(file)?)?;
        Ok((session, archive))
    }
    pub async fn delete_checkpoint(&self, session_id: &str, id: &str) -> Result<()> {
        Uuid::parse_str(id).map_err(|_| CoreError::NotFound)?;
        let deleted = sqlx::query("DELETE FROM checkpoints WHERE id=? AND session_id=?")
            .bind(id)
            .bind(session_id)
            .execute(&self.storage.pool)
            .await?
            .rows_affected();
        if deleted == 0 {
            return Err(CoreError::NotFound);
        }
        let path = self.data_dir.join("checkpoints").join(format!("{id}.json"));
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    pub async fn checkpoint_diff(&self, session_id: &str, id: &str) -> Result<Vec<CheckpointDiff>> {
        let (session, archive) = self.archive(session_id, id).await?;
        let root = Path::new(&session.working_directory);
        let mut result = Vec::new();
        // Only captured files can be restored; missing/skipped files are never guessed deleted.
        for (path, before) in archive.files {
            if !eligible(&path) || crate::redaction::redact(&before) != before {
                continue;
            }
            let after = read_text(root, &path)?;
            if after.as_deref() != Some(&before) {
                result.push(CheckpointDiff {
                    path,
                    before: Some(before),
                    expected: fingerprint(after.as_deref()),
                    after,
                });
            }
        }
        Ok(result)
    }
    pub async fn restore_checkpoint_file(
        &self,
        session_id: &str,
        id: &str,
        path: &str,
        expected: &str,
    ) -> Result<()> {
        let _lease = self.lease(session_id, CancellationToken::new())?;
        let (mut session, archive) = self.archive(session_id, id).await?;
        session.permission_profile = "standard".into();
        let _project = self.project_lease(&session)?;
        if !eligible(path) {
            return Err(invalid("Защищённый путь"));
        }
        let before = archive.files.get(path).ok_or(CoreError::NotFound)?;
        if crate::redaction::redact(before) != *before {
            return Err(invalid("Защищённое содержимое"));
        }
        let root = Path::new(&session.working_directory);
        let current = read_text(root, path)?;
        if fingerprint(current.as_deref()) != expected {
            return Err(invalid(
                "Файл изменён после предпросмотра. Обновите сравнение",
            ));
        }
        // Save a recovery point before replacing anything, never reset Git or the index.
        self.capture_checkpoint(&session, "Перед восстановлением файла")
            .await?;
        let target = crate::files::resolve(root, path, true)?;
        match current {
            Some(current) => crate::files::save(root, path, &current, before)?,
            None => {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(target)?
                    .write_all(before.as_bytes())?;
            }
        }
        Ok(())
    }
    pub async fn branch_chat(&self, session_id: &str, isolated: bool) -> Result<Session> {
        let _lease = self.lease(session_id, CancellationToken::new())?;
        let source = self.storage.session(session_id).await?;
        if source.parent_session_id.is_some() {
            return Err(invalid("Создайте ветку из основного чата"));
        }
        let mut locking = source.clone();
        locking.permission_profile = "standard".into();
        let _project = self.project_lease(&locking)?;
        let native_fork = self
            .engine(&source.provider)?
            .fork_session(&self.account(&source.account_profile_id).await?, &source)
            .await?;
        let mut workspace_id = source.workspace_id.clone();
        let mut worktree_id = None;
        if isolated {
            let root = Path::new(&source.working_directory);
            let files = paths(root).await?;
            let mut total = 0u64;
            let mut copies = Vec::new();
            for path in files {
                let Ok(resolved) = crate::files::resolve(root, &path, false) else {
                    continue;
                };
                let meta = std::fs::metadata(&resolved)?;
                if !meta.is_file() {
                    continue;
                }
                total += meta.len();
                if total > 512 * 1024 * 1024 {
                    return Err(invalid(
                        "Рабочая копия превышает 512 МиБ; подготовьте worktree вручную",
                    ));
                }
                if meta.len() <= 256 * 1024 {
                    if let Ok(text) = std::fs::read_to_string(&resolved) {
                        if crate::redaction::redact(&text) != text {
                            continue;
                        }
                    }
                }
                copies.push((path, resolved));
            }
            let id = Uuid::new_v4().to_string();
            let directory = self.data_dir.join("worktrees").join(&id);
            std::fs::create_dir_all(directory.parent().ok_or(CoreError::NotFound)?)?;
            let name = directory
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_string();
            crate::git::read_git(
                root,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    "--no-checkout",
                    &name,
                    "HEAD",
                ],
            )
            .await?;
            crate::git::read_git(&directory, &["read-tree", "HEAD"]).await?;
            for (relative, source) in copies {
                let target = directory.join(&relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(source, target)?;
            }
            workspace_id = self.add_workspace(&name).await?.id;
            worktree_id = Some(id);
        }
        let mut child = self
            .create_chat(CreateChat {
                workspace_id: Some(workspace_id),
                mode: "single".into(),
                agents: vec![AgentConfig {
                    provider: source.provider.clone(),
                    account_profile_id: source.account_profile_id.clone(),
                    model: source.model.clone(),
                    reasoning_effort: source.reasoning_effort.clone(),
                    fast_mode: false,
                    permission_profile: if source.permission_profile == "full_access" {
                        "standard".into()
                    } else {
                        source.permission_profile.clone()
                    },
                    tools: serde_json::from_str(&source.tool_policy)?,
                    role: source.role.clone(),
                }],
            })
            .await?;
        let context = self.shared_context(&source, None).await?;
        let title = format!(
            "Ветка · {}",
            source.title.chars().take(55).collect::<String>()
        );
        sqlx::query("UPDATE sessions SET title=?,context_summary=?,worktree_id=? WHERE id=?")
            .bind(title)
            .bind(context)
            .bind(worktree_id)
            .bind(&child.id)
            .execute(&self.storage.pool)
            .await?;
        // Copy local public history. Provider-specific native branching is performed separately.
        sqlx::query("INSERT INTO events(session_id,run_id,timestamp,payload) SELECT ?,run_id,timestamp,payload FROM events WHERE session_id=? ORDER BY sequence").bind(&child.id).bind(&source.id).execute(&self.storage.pool).await?;
        child = self.storage.session(&child.id).await?;
        if let Some(upstream) = native_fork {
            self.storage
                .set_provider_session(&child.id, &upstream)
                .await?;
            child = self.storage.session(&child.id).await?;
        }
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockProvider;
    async fn fixture() -> (crate::test_support::TestDirectory, Arc<Core>, Session) {
        let temp = crate::test_support::TestDirectory::new().unwrap();
        let repo = temp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let mut git = std::process::Command::new("git");
        git.args(["init", "--quiet"]).current_dir(&repo);
        assert!(git.status().unwrap().success());
        std::fs::write(repo.join("file.txt"), "original\n").unwrap();
        for args in [
            vec!["add", "file.txt"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        ] {
            assert!(std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        let data = temp.path().join("data");
        std::fs::create_dir(&data).unwrap();
        let core = Core::open_with_providers(&data.join("test.db"), vec![Arc::new(MockProvider)])
            .await
            .unwrap();
        let ws = core.add_workspace(repo.to_str().unwrap()).await.unwrap();
        let session = core
            .create_session(CreateSession {
                workspace_id: ws.id,
                provider: "mock".into(),
                account_profile_id: "mock-local".into(),
                model: "mock-stream-v1".into(),
                permission_profile: "standard".into(),
            })
            .await
            .unwrap();
        (temp, core, session)
    }
    async fn wait_queue(core: &Core, count: usize) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let q = core.task_queue().await.unwrap();
                if q.len() == count
                    && q.iter()
                        .all(|t| !matches!(t.state.as_str(), "queued" | "running"))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn queue_runs_once_in_order_and_configuration_changes_require_confirmation() {
        let (_temp, core, s) = fixture().await;
        core.enqueue_task(&s.id, "First".into()).await.unwrap();
        core.enqueue_task(&s.id, "Second".into()).await.unwrap();
        wait_queue(&core, 2).await;
        assert!(core
            .task_queue()
            .await
            .unwrap()
            .iter()
            .all(|t| t.state == "completed"));
        let prompts: Vec<_> = core
            .storage
            .events(&s.id, None)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|e| {
                if let EventPayload::TurnStarted { prompt } = e.payload {
                    Some(prompt)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(prompts, vec!["First", "Second"]);
        // Hold the worker asleep and change a saved task's configuration before dispatch.
        sqlx::query(
            "INSERT INTO queued_tasks(id,session_id,prompt,configuration,state,run_id,error,created_at) VALUES ('changed',?,'Task','different','queued',NULL,NULL,0)",
        )
        .bind(&s.id)
        .execute(&core.storage.pool)
        .await
        .unwrap();
        core.queue_wake.notify_one();
        wait_queue(&core, 3).await;
        assert!(core
            .task_queue()
            .await
            .unwrap()
            .iter()
            .any(|t| t.id == "changed" && t.state == "paused"));
        core.shutdown().await;
    }
    #[tokio::test]
    async fn long_queued_text_can_be_edited_and_dispatched_in_full() {
        let (_temp, core, s) = fixture().await;
        let lease = core.lease(&s.id, CancellationToken::new()).unwrap();
        let text = format!("START{}END", "длинный текст\n".repeat(5000));
        let id = core.enqueue_task(&s.id, text.clone()).await.unwrap();
        let edited = format!("{text}\nИсправление в конце");
        core.edit_queued_task(&id, edited.clone(), 0).await.unwrap();
        assert_eq!(core.task_queue().await.unwrap()[0].prompt, edited);
        drop(lease);
        core.queue_wake.notify_one();
        wait_queue(&core, 1).await;
        assert!(core
            .storage
            .events(&s.id, None)
            .await
            .unwrap()
            .iter()
            .any(|e| matches!(&e.payload,EventPayload::TurnStarted{prompt} if prompt==&edited)));
        core.shutdown().await;
    }
    #[tokio::test]
    async fn queued_attachments_are_bound_to_original_bytes_and_edits_reorder_pending_tasks() {
        use base64::Engine;
        let (_temp, core, s) = fixture().await;
        let lease = core.lease(&s.id, CancellationToken::new()).unwrap();
        let first = core
            .enqueue_message(
                &s.id,
                "With file".into(),
                vec![crate::attachments::Attachment {
                    name: "note.txt".into(),
                    mime: "text/plain".into(),
                    data: base64::engine::general_purpose::STANDARD.encode("original"),
                }],
            )
            .await
            .unwrap();
        let second = core.enqueue_task(&s.id, "Second".into()).await.unwrap();
        core.edit_queued_task(&second, "Changed second".into(), -1)
            .await
            .unwrap();
        let tasks = core.task_queue().await.unwrap();
        assert_eq!(tasks[0].id, second);
        let saved: Vec<QueuedFile> =
            serde_json::from_str(&tasks.iter().find(|t| t.id == first).unwrap().attachments)
                .unwrap();
        std::fs::write(&saved[0].file.path, "tampered").unwrap();
        drop(lease);
        core.queue_wake.notify_one();
        wait_queue(&core, 2).await;
        let tasks = core.task_queue().await.unwrap();
        assert_eq!(
            tasks.iter().find(|t| t.id == second).unwrap().state,
            "completed"
        );
        assert_eq!(
            tasks.iter().find(|t| t.id == first).unwrap().state,
            "failed"
        );
        let events = core.storage.events(&s.id, None).await.unwrap();
        assert!(!events.iter().any(
            |e| matches!(&e.payload,EventPayload::TurnStarted{prompt} if prompt=="With file")
        ));
        core.shutdown().await;
    }
    #[tokio::test]
    async fn writers_in_same_folder_are_serialized_and_waiting_queue_cancel_does_not_stop_owner() {
        let (_temp, core, s) = fixture().await;
        let other = core
            .create_session(CreateSession {
                workspace_id: s.workspace_id.clone(),
                provider: "mock".into(),
                account_profile_id: "mock-local".into(),
                model: s.model.clone(),
                permission_profile: "standard".into(),
            })
            .await
            .unwrap();
        core.send_message(&s.id, "Manual task".into())
            .await
            .unwrap();
        assert!(matches!(
            core.send_message(&other.id, "Conflict".into()).await,
            Err(CoreError::Busy)
        ));
        let id = core
            .enqueue_task(&other.id, "Waiting".into())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(250)).await;
        core.change_queued_task(&id, false).await.unwrap();
        assert!(!core.runs.lock().unwrap().get(&s.id).unwrap().is_cancelled());
        assert_eq!(core.task_queue().await.unwrap()[0].state, "cancelled");
        core.cancel(&s.id).unwrap();
        core.shutdown().await;
    }
    #[tokio::test]
    async fn checkpoint_restore_uses_compare_and_swap_preserves_git_and_creates_recovery_point() {
        let (_temp, core, s) = fixture().await;
        let point = core.create_checkpoint(&s.id, "Before").await.unwrap();
        let file = Path::new(&s.working_directory).join("file.txt");
        std::fs::write(&file, "modified\n").unwrap();
        let diff = core.checkpoint_diff(&s.id, &point.id).await.unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].before.as_deref(), Some("original\n"));
        std::fs::write(&file, "later user edit\n").unwrap();
        assert!(core
            .restore_checkpoint_file(&s.id, &point.id, "file.txt", &diff[0].expected)
            .await
            .is_err());
        let fresh = core.checkpoint_diff(&s.id, &point.id).await.unwrap();
        core.restore_checkpoint_file(&s.id, &point.id, "file.txt", &fresh[0].expected)
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original\n");
        assert_eq!(core.checkpoints(&s.id).await.unwrap().len(), 2);
        assert!(core
            .restore_checkpoint_file(&s.id, &point.id, "../escape", "missing")
            .await
            .is_err());
        core.delete_checkpoint(&s.id, &point.id).await.unwrap();
        assert_eq!(core.checkpoints(&s.id).await.unwrap().len(), 1);
    }
    #[tokio::test]
    async fn isolated_branch_copies_dirty_files_without_merge_or_source_mutation() {
        let (_temp, core, s) = fixture().await;
        let root = Path::new(&s.working_directory);
        std::fs::write(root.join("file.txt"), "dirty\n").unwrap();
        std::fs::write(root.join("new.txt"), "new\n").unwrap();
        std::fs::write(root.join(".env"), "password=fixture\n").unwrap();
        let fork = core.branch_chat(&s.id, true).await.unwrap();
        assert_ne!(fork.id, s.id);
        assert_ne!(fork.working_directory, s.working_directory);
        assert_eq!(fork.account_profile_id, s.account_profile_id);
        assert!(fork.worktree_id.is_some());
        assert_eq!(
            std::fs::read_to_string(Path::new(&fork.working_directory).join("file.txt")).unwrap(),
            "dirty\n"
        );
        assert!(Path::new(&fork.working_directory).join("new.txt").exists());
        assert!(!Path::new(&fork.working_directory).join(".env").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt")).unwrap(),
            "dirty\n"
        );
    }
    #[tokio::test]
    async fn restart_never_replays_queued_work() {
        let (_temp, core, s) = fixture().await;
        sqlx::query(
            "INSERT INTO queued_tasks(id,session_id,prompt,configuration,state,run_id,error,created_at) VALUES ('pending',?,'Do not replay',?,'queued',NULL,NULL,0)",
        )
        .bind(&s.id)
        .bind(configuration(&s))
        .execute(&core.storage.pool)
        .await
        .unwrap();
        let path = core.data_dir.join("test.db");
        core.shutdown().await;
        drop(core);
        let reopened = Core::open_with_providers(&path, vec![Arc::new(MockProvider)])
            .await
            .unwrap();
        assert_eq!(reopened.task_queue().await.unwrap()[0].state, "paused");
        assert!(reopened
            .storage
            .events(&s.id, None)
            .await
            .unwrap()
            .is_empty());
        reopened.shutdown().await;
    }
}

#[cfg(test)]
mod queue_cancellation_regression {
    use super::*;
    #[tokio::test]
    async fn cancellation_token_for_an_old_queue_item_cannot_cancel_a_new_run() {
        let temp = crate::test_support::TestDirectory::new().unwrap();
        let core = Core::open_with_providers(
            &temp.path().join("queue.db"),
            vec![Arc::new(crate::provider::MockProvider)],
        )
        .await
        .unwrap();
        let ws = core
            .add_workspace(temp.path().to_str().unwrap())
            .await
            .unwrap();
        let session = core
            .create_session(CreateSession {
                workspace_id: ws.id,
                provider: "mock".into(),
                account_profile_id: "mock-local".into(),
                model: "mock-stream-v1".into(),
                permission_profile: "standard".into(),
            })
            .await
            .unwrap();
        let old = CancellationToken::new();
        sqlx::query("INSERT INTO queued_tasks(id,session_id,prompt,configuration,state,run_id,error,created_at) VALUES ('old',?,'old',?,'running','old-run',NULL,0)")
            .bind(&session.id)
            .bind(configuration(&session))
            .execute(&core.storage.pool)
            .await
            .unwrap();
        core.queue_tokens
            .lock()
            .unwrap()
            .insert("old".into(), old.clone());
        core.send_message(&session.id, "New direct run".into())
            .await
            .unwrap();
        core.change_queued_task("old", false).await.unwrap();
        assert!(old.is_cancelled());
        // The mock turn may already have finished and left the registry: then it is not cancelled.
        assert!(!core
            .runs
            .lock()
            .unwrap()
            .get(&session.id)
            .is_some_and(|token| token.is_cancelled()));
        core.cancel(&session.id).unwrap();
        core.shutdown().await;
    }
}
