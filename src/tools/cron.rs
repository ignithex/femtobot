use crate::cron::CronService;
use crate::tools::ToolError;
use rig::completion::request::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;

#[derive(Clone)]
pub struct CronTool {
    service: CronService,
}

impl CronTool {
    pub fn new(service: CronService) -> Self {
        Self { service }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CronArgs {
    /// One of: add, list, remove, status
    pub action: String,
    /// Job name (required for add)
    pub name: Option<String>,
    /// Prompt/message to send when the job runs (required for add)
    pub message: Option<String>,
    /// Schedule for add: cron expression, interval in seconds, or @-style cron
    pub schedule: Option<String>,
    /// Delivery channel for add (e.g. "telegram")
    pub channel: Option<String>,
    /// Delivery target for add (e.g. Telegram chat id)
    pub to: Option<String>,
    /// Job id (required for remove)
    pub id: Option<String>,
}

impl Tool for CronTool {
    const NAME: &'static str = "manage_cron";
    type Args = CronArgs;
    type Output = String;
    type Error = ToolError;

    fn definition(
        &self,
        _prompt: String,
    ) -> impl std::future::Future<Output = ToolDefinition> + Send {
        async {
            ToolDefinition {
                name: Self::NAME.to_string(),
                description: "Manage scheduled tasks. Use action=add for new schedules, list to inspect jobs, remove to delete by id, status for scheduler summary. For add: use schedule as cron expression (e.g. '0 9 * * *'), seconds interval (e.g. '14400' for every 4h), or @-style cron. The message field is the inbound text injected when the job fires. Set channel/to to route the cron turn to a destination context (typically current channel/chat), then use send_message if that turn should notify the user.".to_string(),
                parameters: serde_json::to_value(schemars::schema_for!(CronArgs)).unwrap(),
            }
        }
    }

    fn call(
        &self,
        args: Self::Args,
    ) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        let service = self.service.clone();
        async move {
            let action = args.action.trim().to_lowercase();

            match action.as_str() {
                "add" => {
                    let name = args
                        .name
                        .ok_or_else(|| ToolError::msg("Missing required field: name"))?;
                    let message = args
                        .message
                        .ok_or_else(|| ToolError::msg("Missing required field: message"))?;
                    let schedule = args
                        .schedule
                        .ok_or_else(|| ToolError::msg("Missing required field: schedule"))?;
                    service
                        .add_job(name, schedule, message, args.channel, args.to)
                        .await
                        .map_err(|e| ToolError::msg(e.to_string()))?;
                    Ok("Cron job added.".to_string())
                }
                "list" => {
                    let jobs = service
                        .list_jobs()
                        .await
                        .map_err(|e| ToolError::msg(e.to_string()))?;
                    if jobs.is_empty() {
                        return Ok("No cron jobs found.".to_string());
                    }
                    let mut out = String::new();
                    for job in jobs {
                        let schedule = if job.schedule.kind == "every" {
                            format!("every {}ms", job.schedule.every_ms.unwrap_or(0))
                        } else if job.schedule.kind == "at" {
                            "at".to_string()
                        } else {
                            job.schedule.expr.unwrap_or_else(|| "?".to_string())
                        };
                        let next = job
                            .state
                            .next_run_at_ms
                            .map(|ms| {
                                chrono::DateTime::<chrono::Utc>::from(
                                    std::time::UNIX_EPOCH
                                        + std::time::Duration::from_millis(ms as u64),
                                )
                                .to_rfc3339()
                            })
                            .unwrap_or_else(|| "N/A".to_string());
                        out.push_str(&format!(
                            "{} | {} | {} | {} | next: {}\n",
                            job.id,
                            if job.enabled { "enabled" } else { "disabled" },
                            job.name,
                            schedule,
                            next
                        ));
                    }
                    Ok(out)
                }
                "remove" => {
                    let id = args
                        .id
                        .ok_or_else(|| ToolError::msg("Missing required field: id"))?;
                    let removed = service
                        .remove_job(&id)
                        .await
                        .map_err(|e| ToolError::msg(e.to_string()))?;
                    if removed {
                        Ok("Cron job removed.".to_string())
                    } else {
                        Ok("Cron job not found.".to_string())
                    }
                }
                "status" => {
                    let status = service
                        .status()
                        .await
                        .map_err(|e| ToolError::msg(e.to_string()))?;
                    let next = status
                        .next_wake_at_ms
                        .map(|ms| {
                            chrono::DateTime::<chrono::Utc>::from(
                                std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64),
                            )
                            .to_rfc3339()
                        })
                        .unwrap_or_else(|| "N/A".to_string());
                    Ok(format!(
                        "jobs: {}, enabled: {}, next_wake: {}",
                        status.jobs, status.enabled_jobs, next
                    ))
                }
                _ => Ok("Invalid action. Use: add, list, remove, status.".to_string()),
            }
        }
    }
}
