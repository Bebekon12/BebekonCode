//! Local document preview and app-owned review comments. Never executes Office content.
use crate::{files, model::now, redaction::redact, Core, CoreError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read},
    path::Path,
};

const MAX_FILE: u64 = 20 * 1024 * 1024;
const MAX_XML: u64 = 4 * 1024 * 1024;
const MAX_EXPANDED: usize = 12 * 1024 * 1024;
#[derive(Debug, Serialize)]
pub struct ReviewPart {
    pub name: String,
    pub content: String,
}
#[derive(Debug, Serialize)]
pub struct ReviewDocument {
    pub kind: String,
    pub fingerprint: String,
    pub text: Option<String>,
    pub parts: Vec<ReviewPart>,
}
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReviewComment {
    pub id: String,
    pub workspace_id: String,
    pub path: String,
    pub fingerprint: String,
    pub anchor: String,
    pub quote: String,
    pub body: String,
    pub resolved: bool,
    pub created_at: i64,
}
#[derive(Deserialize)]
pub struct NewComment {
    pub fingerprint: String,
    pub anchor: String,
    pub quote: String,
    pub body: String,
}
fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}

pub fn read(root: &Path, relative: &str) -> Result<ReviewDocument> {
    let path = files::resolve(root, relative, false)?;
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["docx", "xlsx", "csv", "tsv", "md", "txt"].contains(&extension.as_str()) {
        return Err(invalid("Просмотр с комментариями поддерживает DOCX, XLSX, CSV, TSV, Markdown и TXT. PDF и старые DOC/XLS пока недоступны."));
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_FILE {
        return Err(invalid("Документ должен быть файлом до 20 МиБ"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(invalid("Документ превышает 20 МиБ"));
    }
    let fingerprint = format!("{:x}", Sha256::digest(&bytes));
    let mut result = ReviewDocument {
        kind: extension.clone(),
        fingerprint,
        text: None,
        parts: vec![],
    };
    if !["docx", "xlsx"].contains(&extension.as_str()) {
        if bytes.len() > 2 * 1024 * 1024 || bytes.contains(&0) {
            return Err(invalid("Текстовый документ должен быть UTF-8 до 2 МиБ"));
        }
        result.text = Some(String::from_utf8(bytes).map_err(|_| invalid("Нужна кодировка UTF-8"))?);
        return Ok(result);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| invalid("Повреждённый или зашифрованный Office-документ"))?;
    if archive.len() > 2000 {
        return Err(invalid("Слишком много частей документа"));
    }
    let mut expanded = 0;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| invalid("Недоступная часть Office-документа"))?;
        let name = entry.name().to_string();
        let wanted = if extension == "docx" {
            name == "word/document.xml"
        } else {
            [
                "xl/workbook.xml",
                "xl/_rels/workbook.xml.rels",
                "xl/sharedStrings.xml",
            ]
            .contains(&name.as_str())
                || name
                    .strip_prefix("xl/worksheets/sheet")
                    .and_then(|n| n.strip_suffix(".xml"))
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
        };
        if !wanted {
            continue;
        }
        if result.parts.iter().any(|p| p.name == name) {
            return Err(invalid("Повторяющаяся часть Office-документа"));
        }
        if entry.size() > MAX_XML || result.parts.len() >= 67 {
            return Err(invalid(
                "Слишком большая часть документа или больше 64 листов",
            ));
        }
        let mut content = String::new();
        (&mut entry)
            .take(MAX_XML + 1)
            .read_to_string(&mut content)
            .map_err(|_| invalid("Повреждённый XML документа"))?;
        expanded += content.len();
        if content.len() as u64 > MAX_XML
            || expanded > MAX_EXPANDED
            || content.contains("<!DOCTYPE")
            || content.contains("<!ENTITY")
        {
            return Err(invalid("Небезопасный или слишком большой XML документа"));
        }
        result.parts.push(ReviewPart { name, content });
    }
    let required = if extension == "docx" {
        "word/document.xml"
    } else {
        "xl/workbook.xml"
    };
    if !result.parts.iter().any(|p| p.name == required) {
        return Err(invalid("Отсутствует содержимое документа"));
    }
    Ok(result)
}

impl Core {
    pub async fn review_document(&self, workspace: &str, path: &str) -> Result<ReviewDocument> {
        let workspace = self.storage.workspace(workspace).await?;
        let path = path.to_string();
        tokio::task::spawn_blocking(move || read(Path::new(&workspace.root), &path))
            .await
            .map_err(|_| CoreError::Busy)?
    }
    pub async fn review_comments(&self, workspace: &str, path: &str) -> Result<Vec<ReviewComment>> {
        let root = self.storage.workspace(workspace).await?;
        files::resolve(Path::new(&root.root), path, false)?;
        Ok(sqlx::query_as("SELECT * FROM review_comments WHERE workspace_id = ? AND path = ? ORDER BY created_at, id LIMIT 1000").bind(workspace).bind(path).fetch_all(&self.storage.pool).await?)
    }
    pub async fn add_review_comment(
        &self,
        workspace: &str,
        path: &str,
        input: NewComment,
    ) -> Result<ReviewComment> {
        if input.body.trim().is_empty()
            || input.body.len() > 16000
            || input.quote.len() > 4000
            || input.anchor.len() > 256
            || input.anchor.is_empty()
            || input.anchor.chars().any(char::is_control)
        {
            return Err(invalid("Комментарий пустой или слишком большой"));
        }
        let document = self.review_document(workspace, path).await?;
        if document.fingerprint != input.fingerprint {
            return Err(invalid(
                "Файл изменился на диске. Обновите просмотр перед добавлением комментария.",
            ));
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM review_comments WHERE workspace_id = ? AND path = ?",
        )
        .bind(workspace)
        .bind(path)
        .fetch_one(&self.storage.pool)
        .await?;
        if count >= 1000 {
            return Err(invalid("Достигнут лимит 1000 комментариев к файлу"));
        }
        let item = ReviewComment {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: workspace.into(),
            path: path.into(),
            fingerprint: input.fingerprint,
            anchor: input.anchor,
            quote: redact(&input.quote),
            body: redact(input.body.trim()),
            resolved: false,
            created_at: now(),
        };
        sqlx::query("INSERT INTO review_comments VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)")
            .bind(&item.id)
            .bind(&item.workspace_id)
            .bind(&item.path)
            .bind(&item.fingerprint)
            .bind(&item.anchor)
            .bind(&item.quote)
            .bind(&item.body)
            .bind(item.created_at)
            .execute(&self.storage.pool)
            .await?;
        Ok(item)
    }
    pub async fn resolve_review_comment(
        &self,
        workspace: &str,
        id: &str,
        resolved: bool,
    ) -> Result<()> {
        self.storage.workspace(workspace).await?;
        let result = sqlx::query(
            "UPDATE review_comments SET resolved = ? WHERE workspace_id = ? AND id = ?",
        )
        .bind(resolved)
        .bind(workspace)
        .bind(id)
        .execute(&self.storage.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn office(path: &Path, parts: &[(&str, &str)]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, content) in parts {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
    #[test]
    fn bounded_office_reader_rejects_entities_and_escapes() {
        let temp = crate::test_support::TestDirectory::new().unwrap();
        office(
            &temp.path().join("ok.docx"),
            &[
                ("word/document.xml", "<document>Привет</document>"),
                ("../ignored.xml", "never extracted"),
            ],
        );
        let d = read(temp.path(), "ok.docx").unwrap();
        assert_eq!(d.parts.len(), 1);
        assert_eq!(d.fingerprint.len(), 64);
        assert!(read(temp.path(), "../ok.docx").is_err());
        office(
            &temp.path().join("entities.docx"),
            &[(
                "word/document.xml",
                "<!DOCTYPE x [<!ENTITY a SYSTEM 'file:///secret'>]><x>&a;</x>",
            )],
        );
        assert!(read(temp.path(), "entities.docx").is_err());
        office(
            &temp.path().join("big.docx"),
            &[("word/document.xml", &"x".repeat(MAX_XML as usize + 1))],
        );
        assert!(read(temp.path(), "big.docx").is_err());
        office(
            &temp.path().join("ok.xlsx"),
            &[
                ("xl/workbook.xml", "<workbook/>"),
                ("xl/worksheets/sheet1.xml", "<worksheet/>"),
                ("xl/vbaProject.bin", "not loaded"),
            ],
        );
        assert_eq!(read(temp.path(), "ok.xlsx").unwrap().parts.len(), 2);
    }
    #[tokio::test]
    async fn comments_persist_and_bind_to_workspace_and_file_version() {
        let temp = crate::test_support::TestDirectory::new().unwrap();
        let path = temp.path().join("doc.txt");
        std::fs::write(&path, "First version").unwrap();
        let db = temp.path().join("review.db");
        let core = Core::open(&db).await.unwrap();
        let workspace = core
            .add_workspace(temp.path().to_str().unwrap())
            .await
            .unwrap();
        let fingerprint = core
            .review_document(&workspace.id, "doc.txt")
            .await
            .unwrap()
            .fingerprint;
        let input = || NewComment {
            fingerprint: fingerprint.clone(),
            anchor: "p:0".into(),
            quote: "First version".into(),
            body: "Проверь формулировку".into(),
        };
        let c = core
            .add_review_comment(&workspace.id, "doc.txt", input())
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "First version");
        assert!(core
            .resolve_review_comment("another-workspace", &c.id, true)
            .await
            .is_err());
        std::fs::write(&path, "Second version").unwrap();
        assert!(core
            .add_review_comment(&workspace.id, "doc.txt", input())
            .await
            .is_err());
        core.resolve_review_comment(&workspace.id, &c.id, true)
            .await
            .unwrap();
        drop(core);
        let reopened = Core::open(&db).await.unwrap();
        let comments = reopened
            .review_comments(&workspace.id, "doc.txt")
            .await
            .unwrap();
        assert_eq!(comments.len(), 1);
        assert!(comments[0].resolved);
        assert_eq!(comments[0].fingerprint, fingerprint);
    }
}
