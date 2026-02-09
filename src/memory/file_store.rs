use chrono::{Datelike, Local};
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_CONTEXT_TOKENS: usize = 2000;
pub const CHARS_PER_TOKEN: usize = 4;
pub const MAX_CONTEXT_CHARS: usize = MAX_CONTEXT_TOKENS * CHARS_PER_TOKEN;

#[derive(Clone)]
pub struct MemoryStore {
    workspace: PathBuf,
    memory_dir: PathBuf,
    memory_file: PathBuf,
}

impl MemoryStore {
    pub fn new(workspace: PathBuf) -> Self {
        let memory_dir = ensure_dir(&workspace.join("memory"));
        let memory_file = memory_dir.join("MEMORY.md");
        Self {
            workspace,
            memory_dir,
            memory_file,
        }
    }

    pub fn get_today_file(&self) -> PathBuf {
        self.memory_dir.join(format!("{}.md", today_date()))
    }

    pub fn read_today(&self) -> String {
        let today_file = self.get_today_file();
        fs::read_to_string(today_file).unwrap_or_default()
    }

    pub fn read_long_term(&self) -> String {
        fs::read_to_string(&self.memory_file).unwrap_or_default()
    }

    pub fn get_memory_context(&self, max_chars: usize) -> String {
        let mut parts = Vec::new();
        let mut remaining = max_chars;

        let long_term_budget = (max_chars as f64 * 0.6) as usize;
        let long_term = self.read_long_term();
        if !long_term.is_empty() {
            let truncated = truncate(&long_term, long_term_budget);
            parts.push(format!("## Long-term Memory\n{}", truncated));
            remaining = remaining.saturating_sub(truncated.len());
        }

        let today = self.read_today();
        if !today.is_empty() && remaining > 100 {
            let truncated = truncate(&today, remaining);
            parts.push(format!("## Today's Notes\n{}", truncated));
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join("\n\n")
        }
    }

    #[allow(dead_code)]
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }
}

fn ensure_dir(path: &Path) -> PathBuf {
    if let Err(err) = fs::create_dir_all(path) {
        eprintln!("Failed to create dir {}: {}", path.display(), err);
    }
    path.to_path_buf()
}

fn today_date() -> String {
    let now = Local::now().date_naive();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

fn truncate(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }

    let truncate_at = max_chars.saturating_sub(20);
    for sep in ["\n\n", ".\n", ". ", "\n"] {
        if let Some(pos) = content[..truncate_at].rfind(sep) {
            if pos > truncate_at / 2 {
                return format!("{}{}\n... (truncated)", &content[..pos + sep.len()], "");
            }
        }
    }

    if let Some(pos) = content[..truncate_at].rfind(' ') {
        if pos > truncate_at / 2 {
            return format!("{} ... (truncated)", &content[..pos]);
        }
    }

    format!("{}... (truncated)", &content[..truncate_at])
}
