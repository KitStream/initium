mod logging;
mod retry;
mod safety;
mod render;
mod cmd;

use clap::{Parser, Subcommand};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "initium", version, about = "Swiss-army toolbox for Kubernetes initContainers")]
#[command(long_about = "Initium is a multi-tool CLI for Kubernetes initContainers.\nIt provides subcommands to wait for dependencies, run migrations,\nseed databases, render config templates, fetch secrets, and execute\narbitrary commands -- all with safe defaults, structured logging,\nand security guardrails.")]
struct Cli {
    #[arg(long, global = true, help = "Enable JSON log output")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Wait for TCP or HTTP(S) endpoints to become available
    WaitFor {
        #[arg(long, required = true, help = "Target endpoint (tcp://host:port or http(s)://...)")]
        target: Vec<String>,
        #[arg(long, default_value = "300", help = "Overall timeout in seconds")]
        timeout: u64,
        #[arg(long, default_value = "60", help = "Maximum retry attempts")]
        max_attempts: u32,
        #[arg(long, default_value = "1000", help = "Initial delay in milliseconds")]
        initial_delay: u64,
        #[arg(long, default_value = "30000", help = "Maximum delay in milliseconds")]
        max_delay: u64,
        #[arg(long, default_value = "2.0", help = "Backoff multiplier")]
        backoff_factor: f64,
        #[arg(long, default_value = "0.1", help = "Jitter fraction (0.0-1.0)")]
        jitter: f64,
        #[arg(long, default_value = "200", help = "Expected HTTP status code")]
        http_status: u16,
        #[arg(long, help = "Allow insecure TLS connections")]
        insecure_tls: bool,
    },

    /// Run a database migration command with structured logging
    Migrate {
        #[arg(long, default_value = "/work", help = "Working directory")]
        workdir: String,
        #[arg(long, default_value = "", help = "Lock file for idempotency")]
        lock_file: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run a database seed command with structured logging
    Seed {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Render templates into config files
    Render {
        #[arg(long, help = "Path to template file")]
        template: String,
        #[arg(long, help = "Output file path relative to workdir")]
        output: String,
        #[arg(long, default_value = "/work", help = "Working directory")]
        workdir: String,
        #[arg(long, default_value = "envsubst", help = "Template mode: envsubst or gotemplate")]
        mode: String,
    },

    /// Fetch secrets or config from HTTP(S) endpoints
    Fetch {
        #[arg(long, help = "URL to fetch")]
        url: String,
        #[arg(long, help = "Output file path relative to workdir")]
        output: String,
        #[arg(long, default_value = "/work", help = "Working directory")]
        workdir: String,
        #[arg(long, default_value = "", help = "Env var containing auth header")]
        auth_env: String,
        #[arg(long, help = "Skip TLS verification")]
        insecure_tls: bool,
        #[arg(long, help = "Follow HTTP redirects")]
        follow_redirects: bool,
        #[arg(long, help = "Allow cross-site redirects")]
        allow_cross_site_redirects: bool,
        #[arg(long, default_value = "300", help = "Timeout in seconds")]
        timeout: u64,
        #[arg(long, default_value = "3", help = "Max retry attempts")]
        max_attempts: u32,
        #[arg(long, default_value = "1000", help = "Initial delay in ms")]
        initial_delay: u64,
        #[arg(long, default_value = "30000", help = "Max delay in ms")]
        max_delay: u64,
        #[arg(long, default_value = "2.0", help = "Backoff factor")]
        backoff_factor: f64,
        #[arg(long, default_value = "0.1", help = "Jitter fraction")]
        jitter: f64,
    },

    /// Run arbitrary commands with structured logging
    Exec {
        #[arg(long, default_value = "", help = "Working directory")]
        workdir: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let log = logging::Logger::default_logger();
    if cli.json {
        log.set_json(true);
    }

    let result = match cli.command {
        Commands::WaitFor { target, timeout, max_attempts, initial_delay, max_delay, backoff_factor, jitter, http_status, insecure_tls } => {
            let cfg = retry::Config {
                max_attempts,
                initial_delay: Duration::from_millis(initial_delay),
                max_delay: Duration::from_millis(max_delay),
                backoff_factor,
                jitter_fraction: jitter,
            };
            if let Err(e) = cfg.validate() {
                Err(format!("invalid retry config: {}", e))
            } else {
                cmd::wait_for::run(&log, &target, &cfg, Duration::from_secs(timeout), http_status, insecure_tls)
            }
        }
        Commands::Migrate { workdir, lock_file, args } => {
            cmd::migrate::run(&log, &args, &workdir, &lock_file)
        }
        Commands::Seed { args } => {
            cmd::seed::run(&log, &args)
        }
        Commands::Render { template, output, workdir, mode } => {
            cmd::render::run(&log, &template, &output, &workdir, &mode)
        }
        Commands::Fetch { url, output, workdir, auth_env, insecure_tls, follow_redirects, allow_cross_site_redirects, timeout, max_attempts, initial_delay, max_delay, backoff_factor, jitter } => {
            let fetch_cfg = cmd::fetch::Config {
                url, output, workdir, auth_env, insecure_tls,
                follow_redirects, allow_cross_site_redirects,
                timeout: Duration::from_secs(timeout),
            };
            let retry_cfg = retry::Config {
                max_attempts,
                initial_delay: Duration::from_millis(initial_delay),
                max_delay: Duration::from_millis(max_delay),
                backoff_factor,
                jitter_fraction: jitter,
            };
            if let Err(e) = retry_cfg.validate() {
                Err(format!("invalid retry config: {}", e))
            } else {
                cmd::fetch::run(&log, &fetch_cfg, &retry_cfg)
            }
        }
        Commands::Exec { workdir, args } => {
            cmd::exec::run(&log, &args, &workdir)
        }
    };

    if let Err(e) = result {
        log.error(&e, &[]);
        std::process::exit(1);
    }
}

