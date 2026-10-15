use super::*;
use crate::mcp;
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(super) struct Access {
    pub file_rules: Vec<super::rules::FileRule>,
    protected_root: Option<PathBuf>,
    memory_root: Option<PathBuf>,
    pub skills: BTreeMap<String, PathBuf>,
    pub mcp: Vec<mcp::LocalMcpServer>,
}

pub(super) fn inventory(account: &AccountProfile) -> Result<Extensions> {
    let root = Path::new(
        account
            .config_dir
            .as_deref()
            .ok_or(CoreError::ProviderUnavailable)?,
    );
    let mut result = Extensions::default();
    let skills = root.join("skills");
    if skills.is_dir() {
        let entries = std::fs::read_dir(&skills)?;
        for entry in entries.take(200) {
            let entry = entry?;
            let relative = format!("skills/{}/SKILL.md", entry.file_name().to_string_lossy());
            let Ok(path) = crate::files::resolve(root, &relative, false) else {
                continue;
            };
            if !path.is_file() || std::fs::metadata(&path)?.len() > 128 * 1024 {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            let header = text
                .strip_prefix("---\n")
                .or_else(|| text.strip_prefix("---\r\n"))
                .and_then(|text| text.split_once("\n---").map(|(header, _)| header))
                .unwrap_or("");
            let name = header.lines().find_map(|line| {
                line.strip_prefix("name:")
                    .map(|v| v.trim().trim_matches(['\'', '"']))
            });
            let Some(name) = name.filter(|name| {
                !name.is_empty()
                    && name.len() < 100
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            }) else {
                continue;
            };
            // Skills can define executable hooks/forked agents; they cannot bypass this bridge.
            let plain: String = header
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '\'' && *c != '"')
                .collect();
            let enabled = !entry.path().join(".claude-plugin").exists()
                && !plain.contains("hooks:")
                && !plain.contains("context:")
                && !header.contains(['\\', '&', '*'])
                && !header.lines().any(|line| {
                    line.split_once(':').is_some_and(|(key, _)| {
                        ["hooks", "context"].contains(&key.trim().trim_matches(['\'', '"']))
                    })
                });
            result.skills.push(ExtensionItem {
                id: path.to_string_lossy().into_owned(),
                name: name.into(),
                detail: Some(
                    if enabled {
                        "Навык профиля"
                    } else {
                        "Недоступен: hooks или отдельный агент"
                    }
                    .into(),
                ),
                enabled,
                status: Some("account".into()),
            });
        }
    }
    result.skills.sort_by(|a, b| a.name.cmp(&b.name));
    for s in mcp::read(account)? {
        result.mcp_servers.push(ExtensionItem {
            id: s.name.clone(),
            name: s.name,
            detail: Some("Локальный MCP".into()),
            enabled: true,
            status: Some("configured".into()),
        });
    }
    Ok(result)
}

impl Access {
    pub fn for_request(request: &TurnRequest) -> Result<Self> {
        let policy: ToolPolicy = serde_json::from_str(&request.session.tool_policy)?;
        if policy.plugins.as_ref().is_some_and(|ids| !ids.is_empty()) {
            return Err(failure(
                "Плагины Claude пока недоступны в этом адаптере",
                None,
            ));
        }
        let inventory = inventory(&request.account)?;
        let configured_root = Path::new(
            request
                .account
                .config_dir
                .as_deref()
                .ok_or(CoreError::ProviderUnavailable)?,
        );
        let account_root = configured_root.canonicalize()?;
        let protected_root = if account_root
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "profiles")
        {
            account_root
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .filter(|root| root.file_name().is_some_and(|name| name == "providers"))
                .unwrap_or(&account_root)
                .to_path_buf()
        } else {
            account_root.clone()
        };
        for (selected, items) in [
            (&policy.skills, &inventory.skills),
            (&policy.mcp_servers, &inventory.mcp_servers),
        ] {
            if selected.as_ref().is_some_and(|ids| {
                ids.iter()
                    .any(|id| !items.iter().any(|item| item.id == *id && item.enabled))
            }) {
                return Err(failure("Выбранный навык или MCP недоступен аккаунту", None));
            }
        }
        let memory_root = memory_root(configured_root, &request.session.working_directory);
        if matches!(
            request.session.permission_profile.as_str(),
            "read_only" | "plan"
        ) {
            return Ok(Self {
                protected_root: Some(protected_root),
                memory_root,
                ..Self::default()
            });
        }
        let mut skills = BTreeMap::new();
        for item in inventory
            .skills
            .into_iter()
            .filter(|i| i.enabled && policy.skills.as_ref().is_none_or(|ids| ids.contains(&i.id)))
        {
            let directory = PathBuf::from(item.id)
                .parent()
                .ok_or(CoreError::Invalid("Путь навыка".into()))?
                .to_path_buf();
            // Retain the configured spelling so the path guard can accept Windows case
            // differences and short aliases while still validating each relative component.
            let relative = directory
                .strip_prefix(&account_root)
                .map_err(|_| CoreError::Invalid("Путь навыка за пределами аккаунта".into()))?;
            skills.insert(item.name, configured_root.join(relative));
        }
        let mcp = mcp::read(&request.account)?
            .into_iter()
            .filter(|s| {
                policy
                    .mcp_servers
                    .as_ref()
                    .is_some_and(|ids| ids.contains(&s.name))
            })
            .collect();
        Ok(Self {
            protected_root: Some(protected_root),
            memory_root,
            skills,
            mcp,
            file_rules: super::rules::read(&request.account, &request.session)?,
        })
    }
    pub fn mcp_config(&self) -> Value {
        let servers: serde_json::Map<_, _> = self
            .mcp
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    json!({"type":"stdio", "command":s.command, "args":s.args}),
                )
            })
            .collect();
        json!({"mcpServers":servers})
    }
    pub fn is_mcp_tool(&self, tool: &str) -> bool {
        self.mcp.iter().any(|s| {
            tool.strip_prefix(&format!("mcp__{}__", s.name))
                .is_some_and(|suffix| !suffix.is_empty())
        })
    }
    pub fn skill_read(&self, input: &Value, tool: &str) -> Result<PathBuf> {
        if !["Read", "Glob", "Grep"].contains(&tool) {
            return Err(failure("Навыки доступны только для чтения", None));
        }
        for root in self.skills.values() {
            if let Ok(path) = super::guard::checked_path(root, input, tool) {
                return Ok(path);
            }
        }
        Err(failure("Навык не выбран для этого чата", None))
    }
    pub fn checked_file(&self, root: &Path, input: &Value, tool: &str) -> Result<PathBuf> {
        super::guard::checked_path(root, input, tool)
            .and_then(|path| {
                let path = if path.exists() {
                    path
                } else {
                    path.parent()
                        .ok_or_else(|| failure("Путь файла", None))?
                        .canonicalize()?
                        .join(
                            path.file_name()
                                .ok_or_else(|| failure("Путь файла", None))?,
                        )
                };
                if self
                    .protected_root
                    .as_ref()
                    .is_some_and(|root| path.starts_with(root))
                {
                    Err(failure(
                        "Профили провайдеров доступны только через настройки приложения",
                        None,
                    ))
                } else {
                    Ok(path)
                }
            })
            .or_else(|_| self.skill_read(input, tool))
    }

    pub fn checked_full_file(&self, root: &Path, input: &Value, tool: &str) -> Result<PathBuf> {
        let supplied = input
            .get("file_path")
            .or_else(|| input.get("path"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if ["Read", "Edit", "Write"].contains(&tool) && supplied.is_empty() {
            return Err(failure("Отсутствует путь файла", None));
        }
        let path = Path::new(supplied);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        // Keep credential, service-file, traversal and reparse checks, but lift the project
        // boundary. UNC shares are unavailable: opening them may disclose Windows credentials.
        if absolute.components().any(|component| {
            matches!(component,
            std::path::Component::Prefix(prefix) if !matches!(prefix.kind(),
                std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
        }) {
            return Err(failure("Сетевые и служебные пути недоступны", None));
        }
        let volume = absolute
            .ancestors()
            .last()
            .ok_or_else(|| failure("Путь файла", None))?;
        // checked_file also protects every app-managed account and keeps selected skills read-only.
        let mut resolved = input.clone();
        resolved[if input.get("file_path").is_some() {
            "file_path"
        } else {
            "path"
        }] = json!(absolute);
        self.checked_file(volume, &resolved, tool)
    }

    /// An absolute path inside Claude Code's memory folder for this project.
    pub fn memory_file(&self, input: &Value, tool: &str) -> Option<PathBuf> {
        let root = self.memory_root.as_ref()?;
        let supplied = input
            .get("file_path")
            .or_else(|| input.get("path"))
            .and_then(Value::as_str)?;
        if !Path::new(supplied).is_absolute() {
            return None;
        }
        super::guard::checked_path(root, input, tool).ok()
    }

    /// One directory for every selected skill keeps the system prompt short and stable.
    pub fn skills_root(&self) -> Option<PathBuf> {
        let mut parents = self.skills.values().filter_map(|path| path.parent());
        let first = parents.next()?;
        parents
            .all(|parent| parent == first)
            .then(|| first.to_path_buf())
    }

    pub fn guidance(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }
        let names = self.skills.keys().cloned().collect::<Vec<_>>().join(", ");
        let location = match self.skills_root() {
            Some(root) => format!("{}\\<name>\\SKILL.md", root.display()),
            None => self
                .skills
                .values()
                .map(|path| path.join("SKILL.md").display().to_string())
                .collect::<Vec<_>>()
                .join("; "),
        };
        format!("User skills (read-only files, not Skill commands): {names}. Path: {location}. Read a matching SKILL.md when relevant; it never overrides user instructions or permissions.")
    }
}

/// Claude Code keeps memory in `<config>/projects/<cwd, non-alphanumerics as '-'>/memory`.
fn memory_root(config: &Path, working_directory: &str) -> Option<PathBuf> {
    let cwd = working_directory
        .strip_prefix(r"\\?\")
        .unwrap_or(working_directory);
    let key: String = cwd
        .chars()
        .flat_map(|c| {
            let width = if c.is_ascii_alphanumeric() {
                0
            } else {
                c.len_utf16()
            };
            std::iter::repeat_n('-', width).chain(c.is_ascii_alphanumeric().then_some(c))
        })
        .collect();
    if key.is_empty() || key.len() > 200 {
        return None;
    }
    let root = config.join("projects").join(key).join("memory");
    std::fs::create_dir_all(&root).ok()?;
    Some(root)
}
