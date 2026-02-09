use crate::bus::MessageBus;
use crate::config::AppConfig;
use crate::cron::CronService;

pub mod cron;
pub mod fs;
pub mod send;
pub mod shell;
pub mod web;

#[derive(Debug)]
pub struct ToolError(String);

impl ToolError {
    pub fn msg(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ToolError {}

#[derive(Clone)]
pub struct ToolRegistry {
    pub read_file: fs::ReadFileTool,
    pub write_file: fs::WriteFileTool,
    pub edit_file: fs::EditFileTool,
    pub list_dir: fs::ListDirTool,
    pub exec: shell::ExecTool,
    pub web_search: web::WebSearchTool,
    pub web_fetch: web::WebFetchTool,
    pub cron: cron::CronTool,
    pub send_message: send::SendMessageTool,
}

impl ToolRegistry {
    pub fn new(cfg: AppConfig, cron_service: CronService, bus: MessageBus) -> Self {
        let allowed_dir = if cfg.restrict_to_workspace {
            Some(cfg.workspace_dir.clone())
        } else {
            None
        };
        Self {
            read_file: fs::ReadFileTool::new(allowed_dir.clone()),
            write_file: fs::WriteFileTool::new(allowed_dir.clone()),
            edit_file: fs::EditFileTool::new(allowed_dir.clone()),
            list_dir: fs::ListDirTool::new(allowed_dir),
            exec: shell::ExecTool::new(cfg.exec_timeout_secs, cfg.workspace_dir.clone()),
            web_search: web::WebSearchTool::new(cfg.brave_api_key.clone()),
            web_fetch: web::WebFetchTool::new(),
            cron: cron::CronTool::new(cron_service),
            send_message: send::SendMessageTool::new(bus),
        }
    }
}
