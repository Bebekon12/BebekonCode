//! Bounded collaboration: persistent participants, explicit permissions, serialized writers.
use crate::{model::*, Core, Result};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) fn short_continuation(prompt: &str) -> bool {
    let text = prompt.trim().trim_end_matches(['.', '!']).to_lowercase();
    matches!(
        text.as_str(),
        "да" | "ок"
            | "хорошо"
            | "спасибо"
            | "продолжай"
            | "продолжи"
            | "делай"
            | "реализуй"
            | "continue"
            | "yes"
            | "ok"
            | "thanks"
    )
}
pub(crate) fn preferred_scope(role: &str, index: usize) -> usize {
    let role = role.to_lowercase();
    if ["реценз", "тест", "провер", "риски", "review"]
        .iter()
        .any(|word| role.contains(word))
    {
        1
    } else if ["архит", "ux", "дизайн", "механ", "интерфейс"]
        .iter()
        .any(|word| role.contains(word))
    {
        0
    } else {
        index.min(1)
    }
}
pub(crate) fn scope(index: usize, role: &str, prompt: &str) -> String {
    let lower = prompt.to_lowercase();
    let domain = if index == 0 {
        if ["unity", "патрон", "шейдер", "vfx", "ледян", "игрок"]
            .iter()
            .any(|word| lower.contains(word))
        {
            "Механика, UX и визуальная концепция: состояния ввода и персонажа, переходы, подход к VFX/анимации и интерфейсы между ними. Не повторяй реализацию и тестирование ведущего."
        } else {
            "Архитектура, интерфейсы и UX: найди минимальные точки интеграции и предложи конкретные контракты. Не выполняй реализацию и не повторяй тестирование ведущего."
        }
    } else {
        "Независимая проверка рисков: граничные случаи, совместимость, безопасность, производительность и воспроизводимые критерии приёмки. Не повторяй архитектурные варианты: ищи дефекты и способы их проверить."
    };
    format!("{domain}\nУчитывай выбранную пользователем специализацию: {}. Только твоя область; не выполняй весь запрос заново. Сначала найди релевантные пути, читай лишь нужные фрагменты. Не запускай сборки, тесты, MCP или других агентов. Верни короткую записку: проверенные факты и пути, решение/контракт, риски и необходимые проверки; ориентир до 2500 символов, без полного пересказа задачи и кода. Если данных нет, прямо укажи неизвестное.",serde_json::to_string(role).unwrap_or_default())
}
pub(crate) fn manual_scope(role: &str, permission: &str) -> String {
    let action = if matches!(permission, "read_only" | "plan") {
        "Работай только на чтение: передай факты, конкретное предложение и критерии проверки."
    } else {
        "Выполни только свою часть запрошенной работы в пределах выбранных прав. Используй назначенные инструменты по необходимости. Меняй только относящиеся к своей области файлы; не отменяй чужие правки. Если пользователь просит только варианты или анализ, не начинай реализацию. Передай сделанные изменения, пути файлов и результаты проверок."
    };
    format!("Твоя единственная область назначена пользователем: {}. Не выполняй весь запрос заново и не дублируй другие роли. {action} Не запускай вложенных агентов. Верни ведущему короткий отчёт (ориентир 2500 символов); не выдумывай результаты.",serde_json::to_string(role).unwrap_or_default())
}

/// Keep both decisions at the beginning and the final result; never truncate UTF-8 mid-codepoint.
pub(crate) fn bounded(text: &str, budget: usize) -> String {
    if text.len() <= budget {
        return text.into();
    }
    let mut head = budget * 2 / 3;
    while !text.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = text.len() - budget / 3;
    while !text.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n[Средняя часть сокращена; полная запись в истории участника]\n{}",
        &text[..head],
        &text[tail..]
    )
}
impl Core {
    pub(crate) async fn specialist_lane(
        &self,
        root: &Session,
        template: &Session,
        domain: &str,
        title: &str,
        context: &str,
    ) -> Result<Session> {
        let key = format!(
            "{}:{:x}",
            template.id,
            Sha256::digest(
                json!([
                    template.provider,
                    template.account_profile_id,
                    template.model,
                    template.reasoning_effort,
                    template.role,
                    template.permission_profile,
                    template.tool_policy,
                    template.fast_mode,
                    template.conversation_settings,
                    root.chat_mode,
                    domain
                ])
                .to_string()
                .as_bytes()
            )
        );
        let stored: Option<String> =
            sqlx::query_scalar("SELECT session_id FROM team_lanes WHERE root_id=? AND lane_key=?")
                .bind(&root.id)
                .bind(&key)
                .fetch_optional(&self.storage.pool)
                .await?;
        if let Some(id) = stored {
            return self.storage.session(&id).await;
        }
        let id = Uuid::new_v4().to_string();
        let manual = root.chat_mode == "team"
            || !matches!(template.permission_profile.as_str(), "read_only" | "plan");
        let permission = template.permission_profile.as_str();
        let tools = if manual {
            template.tool_policy.clone()
        } else {
            serde_json::to_string(&ToolPolicy {
                plugins: Some(vec![]),
                mcp_servers: Some(vec![]),
                skills: Some(vec![]),
            })?
        };
        let mut tx = self.storage.pool.begin().await?;
        sqlx::query("INSERT INTO sessions(id,workspace_id,provider,account_profile_id,model,title,status,permission_profile,working_directory,created_at,updated_at,reasoning_effort,tool_policy,parent_session_id,chat_mode,role,context_summary,fast_mode,conversation_settings) VALUES (?,?,?,?,?,?,'idle',?,?,?,?,?,?,?,'task',?,?,?,?)")
            .bind(&id).bind(&root.workspace_id).bind(&template.provider).bind(&template.account_profile_id).bind(&template.model).bind(title).bind(permission).bind(&root.working_directory).bind(now()).bind(now()).bind(&template.reasoning_effort).bind(tools).bind(&root.id).bind(&template.role).bind(bounded(context,8000)).bind(root.chat_mode=="team"&&template.fast_mode).bind(&template.conversation_settings).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO team_lanes VALUES (?,?,?)")
            .bind(&root.id)
            .bind(key)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.storage.session(&id).await
    }
    pub(crate) async fn last_team_result(&self, root: &Session) -> Result<String> {
        let run:Option<String>=sqlx::query_scalar("SELECT run_id FROM events WHERE session_id=? AND json_extract(payload,'$.type')='turn_completed' ORDER BY sequence DESC LIMIT 1").bind(&root.id).fetch_optional(&self.storage.pool).await?;
        let Some(run) = run else {
            return Ok(String::new());
        };
        let fragments:Vec<String>=sqlx::query_scalar("SELECT json_extract(payload,'$.text') FROM events WHERE session_id=? AND run_id=? AND json_extract(payload,'$.type')='assistant_text_delta' ORDER BY sequence DESC LIMIT 500").bind(&root.id).bind(run).fetch_all(&self.storage.pool).await?;
        let mut text = String::new();
        for fragment in fragments.into_iter().rev() {
            text.push_str(&fragment);
            if text.len() > 6000 {
                text = bounded(&text, 6000);
            }
        }
        Ok(text)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_do_not_duplicate_implementation_or_each_other() {
        let first = scope(0, "Архитектор", "Unity патроны и ледяной эффект");
        let second = scope(1, "Рецензент", "Unity патроны и ледяной эффект");
        assert!(first.contains("Механика, UX"));
        assert!(second.contains("Независимая проверка"));
        assert_ne!(first, second);
        assert!(short_continuation("Продолжай!"));
        assert!(!short_continuation(
            "Продолжай: сначала проверь новую механику"
        ));
        let long = format!("Начало{}Конец", "д".repeat(10000));
        let short = bounded(&long, 8000);
        assert!(short.starts_with("Начало"));
        assert!(short.ends_with("Конец"));
        assert!(short.len() < 8200);
    }
}
