mod agent;
mod bus;
mod config;
mod configure;
mod cron;
mod memory;
mod session_compaction;
mod telegram;
mod tools;
mod transcription;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "femtobot", version, about = "femtobot CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Run,
    Configure,
    Cron {
        /// Admin cron operations (tool-driven scheduling is preferred)
        #[command(subcommand)]
        command: CronCommands,
    },
}

#[derive(Subcommand)]
enum CronCommands {
    List,
    Status,
    Remove {
        #[arg(long)]
        id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => run().await,
        Commands::Configure => configure::run(),
        Commands::Cron { command } => handle_cron(command).await,
    }
}

async fn run() -> Result<()> {
    let cfg = config::AppConfig::load()?;

    let (bus, bus_handle) = bus::MessageBus::new();

    // Start Cron Service
    let cron_service = cron::CronService::new(&cfg, bus.clone());
    cron_service.start().await;

    let agent = agent::AgentLoop::new(cfg.clone(), bus.clone(), cron_service.clone());
    tokio::spawn(async move {
        agent.run().await;
    });

    telegram::start(cfg, bus, bus_handle).await?;

    Ok(())
}

async fn handle_cron(cmd: CronCommands) -> Result<()> {
    let cfg = config::AppConfig::load()?;
    // We don't need a real bus for CLI operations acting on the store
    let (bus, _) = bus::MessageBus::new();
    let service = cron::CronService::new(&cfg, bus);

    match cmd {
        CronCommands::List => {
            let jobs = service.list_jobs().await?;
            if jobs.is_empty() {
                println!("No cron jobs found.");
            } else {
                println!(
                    "{:<10} {:<20} {:<20} {:<10} {:<20}",
                    "ID", "Name", "Schedule", "Status", "Next Run"
                );
                println!("{:-<80}", "");
                for job in jobs {
                    let next = job
                        .state
                        .next_run_at_ms
                        .map(|ms| {
                            chrono::DateTime::<chrono::Utc>::from(
                                std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64),
                            )
                            .to_rfc3339()
                        })
                        .unwrap_or_else(|| "N/A".to_string());
                    let schedule_str = if job.schedule.kind == "every" {
                        format!("every {}ms", job.schedule.every_ms.unwrap_or(0))
                    } else if job.schedule.kind == "at" {
                        "at specific time".to_string()
                    } else {
                        job.schedule.expr.clone().unwrap_or("?".to_string())
                    };

                    println!(
                        "{:<10} {:<20} {:<20} {:<10} {:<20}",
                        job.id,
                        job.name,
                        schedule_str,
                        if job.enabled { "Enabled" } else { "Disabled" },
                        next
                    );
                }
            }
        }
        CronCommands::Status => {
            let status = service.status().await?;
            let next = status
                .next_wake_at_ms
                .map(|ms| {
                    chrono::DateTime::<chrono::Utc>::from(
                        std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64),
                    )
                    .to_rfc3339()
                })
                .unwrap_or_else(|| "N/A".to_string());
            println!("Jobs: {}", status.jobs);
            println!("Enabled jobs: {}", status.enabled_jobs);
            println!("Next wake: {}", next);
        }
        CronCommands::Remove { id } => match service.remove_job(&id).await {
            Ok(true) => println!("Job removed."),
            Ok(false) => println!("Job not found."),
            Err(e) => println!("Error removing job: {}", e),
        },
    }
    Ok(())
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
