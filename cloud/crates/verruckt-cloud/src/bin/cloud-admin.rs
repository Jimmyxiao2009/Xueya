use anyhow::Context;
use clap::{Parser, Subcommand};
use verruckt_cloud::config::Config;
use verruckt_cloud::db::connect_and_migrate;
use verruckt_cloud::services::admin;
use verruckt_cloud::state::AppState;

#[derive(Debug, Parser)]
#[command(name = "cloud-admin", about = "Verruckt Cloud administration CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a user. Optionally assign a plan such as free or pro.
    CreateUser {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: String,
        #[arg(long)]
        plan: Option<String>,
    },
    /// Change a user's plan.
    SetPlan {
        #[arg(long)]
        email: String,
        #[arg(long)]
        plan: String,
    },
    /// Show a user's account summary.
    ShowUser {
        #[arg(long)]
        email: String,
    },
    /// Reset monthly usage counters and release active AI reservations.
    ResetUsage {
        #[arg(long)]
        email: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let config = Config::from_env().context("invalid cloud configuration")?;
    let pool = connect_and_migrate(&config.database_url).await?;
    let state = AppState::new(pool, config, None, None);

    match cli.command {
        Command::CreateUser {
            email,
            password,
            plan,
        } => {
            let user_id = admin::create_user(&state, &email, &password, plan.as_deref()).await?;
            println!("created user {user_id} ({email})");
        }
        Command::SetPlan { email, plan } => {
            let user_id = admin::set_plan(&state, &email, &plan).await?;
            println!("set plan for {email} ({user_id}) to {plan}");
        }
        Command::ShowUser { email } => {
            let report = admin::show_user(&state, &email).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "id": report.id,
                    "email": report.email,
                    "plan": report.plan,
                    "devices": report.devices,
                    "created_at": report.created_at,
                }))?
            );
        }
        Command::ResetUsage { email } => {
            let user_id = admin::reset_usage(&state, &email).await?;
            println!("reset usage for {email} ({user_id})");
        }
    }
    Ok(())
}
