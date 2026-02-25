mod cmd;
mod duration;
mod logging;
mod render;
mod retry;
mod safety;
mod seed;
mod template_funcs;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "initium",
    version,
    about = "Swiss-army toolbox for Kubernetes initContainers"
)]
#[command(
    long_about = "Initium is a multi-tool CLI for Kubernetes initContainers.\nIt provides subcommands to wait for dependencies, run migrations,\nseed databases, render config templates, fetch secrets, and execute\narbitrary commands -- all with safe defaults, structured logging,\nand security guardrails."
)]
struct Cli {
    #[arg(
        long,
        global = true,
        env = "INITIUM_JSON",
        help = "Enable JSON log output"
    )]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Wait for TCP or HTTP(S) endpoints to become available
    WaitFor {
        #[arg(
            long,
            required = true,
            env = "INITIUM_TARGET",
            value_delimiter = ',',
            help = "Target endpoint (tcp://host:port or http(s)://...)"
        )]
        target: Vec<String>,
        #[arg(
            long,
            default_value = "5m",
            env = "INITIUM_TIMEOUT",
            help = "Overall timeout (e.g. 30s, 5m, 1h)"
        )]
        timeout: String,
        #[arg(
            long,
            default_value = "60",
            env = "INITIUM_MAX_ATTEMPTS",
            help = "Maximum retry attempts"
        )]
        max_attempts: u32,
        #[arg(
            long,
            default_value = "1s",
            env = "INITIUM_INITIAL_DELAY",
            help = "Initial retry delay (e.g. 500ms, 1s, 5s)"
        )]
        initial_delay: String,
        #[arg(
            long,
            default_value = "30s",
            env = "INITIUM_MAX_DELAY",
            help = "Maximum retry delay (e.g. 10s, 30s, 1m)"
        )]
        max_delay: String,
        #[arg(
            long,
            default_value = "2.0",
            env = "INITIUM_BACKOFF_FACTOR",
            help = "Backoff multiplier"
        )]
        backoff_factor: f64,
        #[arg(
            long,
            default_value = "0.1",
            env = "INITIUM_JITTER",
            help = "Jitter fraction (0.0-1.0)"
        )]
        jitter: f64,
        #[arg(
            long,
            default_value = "200",
            env = "INITIUM_HTTP_STATUS",
            help = "Expected HTTP status code"
        )]
        http_status: u16,
        #[arg(
            long,
            env = "INITIUM_INSECURE_TLS",
            help = "Allow insecure TLS connections"
        )]
        insecure_tls: bool,
    },

    /// Run a database migration command with structured logging
    Migrate {
        #[arg(
            long,
            default_value = "/work",
            env = "INITIUM_WORKDIR",
            help = "Working directory"
        )]
        workdir: String,
        #[arg(
            long,
            default_value = "",
            env = "INITIUM_LOCK_FILE",
            help = "Lock file for idempotency"
        )]
        lock_file: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Apply structured database seeds from a YAML/JSON spec file
    Seed {
        #[arg(
            long,
            required = true,
            env = "INITIUM_SPEC",
            help = "Path to seed spec file (YAML or JSON)"
        )]
        spec: String,
        #[arg(
            long,
            env = "INITIUM_RESET",
            help = "Reset mode: delete existing data before re-seeding"
        )]
        reset: bool,
    },

    /// Render templates into config files
    Render {
        #[arg(
            long,
            required = true,
            env = "INITIUM_TEMPLATE",
            help = "Path to template file"
        )]
        template: String,
        #[arg(
            long,
            required = true,
            env = "INITIUM_OUTPUT",
            help = "Output file path relative to workdir"
        )]
        output: String,
        #[arg(
            long,
            default_value = "/work",
            env = "INITIUM_WORKDIR",
            help = "Working directory"
        )]
        workdir: String,
        #[arg(
            long,
            default_value = "envsubst",
            env = "INITIUM_MODE",
            help = "Template mode: envsubst or gotemplate"
        )]
        mode: String,
    },

    /// Fetch secrets or config from HTTP(S) endpoints
    Fetch {
        #[arg(long, required = true, env = "INITIUM_URL", help = "URL to fetch")]
        url: String,
        #[arg(
            long,
            required = true,
            env = "INITIUM_OUTPUT",
            help = "Output file path relative to workdir"
        )]
        output: String,
        #[arg(
            long,
            default_value = "/work",
            env = "INITIUM_WORKDIR",
            help = "Working directory"
        )]
        workdir: String,
        #[arg(
            long,
            default_value = "",
            env = "INITIUM_AUTH_ENV",
            help = "Env var containing auth header"
        )]
        auth_env: String,
        #[arg(long, env = "INITIUM_INSECURE_TLS", help = "Skip TLS verification")]
        insecure_tls: bool,
        #[arg(long, env = "INITIUM_FOLLOW_REDIRECTS", help = "Follow HTTP redirects")]
        follow_redirects: bool,
        #[arg(
            long,
            env = "INITIUM_ALLOW_CROSS_SITE_REDIRECTS",
            help = "Allow cross-site redirects"
        )]
        allow_cross_site_redirects: bool,
        #[arg(
            long,
            default_value = "5m",
            env = "INITIUM_TIMEOUT",
            help = "Overall timeout (e.g. 30s, 5m, 1h)"
        )]
        timeout: String,
        #[arg(
            long,
            default_value = "3",
            env = "INITIUM_MAX_ATTEMPTS",
            help = "Max retry attempts"
        )]
        max_attempts: u32,
        #[arg(
            long,
            default_value = "1s",
            env = "INITIUM_INITIAL_DELAY",
            help = "Initial retry delay (e.g. 500ms, 1s, 5s)"
        )]
        initial_delay: String,
        #[arg(
            long,
            default_value = "30s",
            env = "INITIUM_MAX_DELAY",
            help = "Maximum retry delay (e.g. 10s, 30s, 1m)"
        )]
        max_delay: String,
        #[arg(
            long,
            default_value = "2.0",
            env = "INITIUM_BACKOFF_FACTOR",
            help = "Backoff factor"
        )]
        backoff_factor: f64,
        #[arg(
            long,
            default_value = "0.1",
            env = "INITIUM_JITTER",
            help = "Jitter fraction"
        )]
        jitter: f64,
    },

    /// Run arbitrary commands with structured logging
    Exec {
        #[arg(
            long,
            default_value = "",
            env = "INITIUM_WORKDIR",
            help = "Working directory"
        )]
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
        Commands::WaitFor {
            target,
            timeout,
            max_attempts,
            initial_delay,
            max_delay,
            backoff_factor,
            jitter,
            http_status,
            insecure_tls,
        } => (|| {
            let timeout_dur = duration::parse_duration(&timeout)
                .map_err(|e| format!("invalid --timeout: {}", e))?;
            let initial_delay_dur = duration::parse_duration(&initial_delay)
                .map_err(|e| format!("invalid --initial-delay: {}", e))?;
            let max_delay_dur = duration::parse_duration(&max_delay)
                .map_err(|e| format!("invalid --max-delay: {}", e))?;
            let cfg = retry::Config {
                max_attempts,
                initial_delay: initial_delay_dur,
                max_delay: max_delay_dur,
                backoff_factor,
                jitter_fraction: jitter,
            };
            cfg.validate()
                .map_err(|e| format!("invalid retry config: {}", e))?;
            cmd::wait_for::run(&log, &target, &cfg, timeout_dur, http_status, insecure_tls)
        })(),
        Commands::Migrate {
            workdir,
            lock_file,
            args,
        } => cmd::migrate::run(&log, &args, &workdir, &lock_file),
        Commands::Seed { spec, reset } => seed::run(&log, &spec, reset),
        Commands::Render {
            template,
            output,
            workdir,
            mode,
        } => cmd::render::run(&log, &template, &output, &workdir, &mode),
        Commands::Fetch {
            url,
            output,
            workdir,
            auth_env,
            insecure_tls,
            follow_redirects,
            allow_cross_site_redirects,
            timeout,
            max_attempts,
            initial_delay,
            max_delay,
            backoff_factor,
            jitter,
        } => (|| {
            let timeout_dur = duration::parse_duration(&timeout)
                .map_err(|e| format!("invalid --timeout: {}", e))?;
            let initial_delay_dur = duration::parse_duration(&initial_delay)
                .map_err(|e| format!("invalid --initial-delay: {}", e))?;
            let max_delay_dur = duration::parse_duration(&max_delay)
                .map_err(|e| format!("invalid --max-delay: {}", e))?;
            let fetch_cfg = cmd::fetch::Config {
                url,
                output,
                workdir,
                auth_env,
                insecure_tls,
                follow_redirects,
                allow_cross_site_redirects,
                timeout: timeout_dur,
            };
            let retry_cfg = retry::Config {
                max_attempts,
                initial_delay: initial_delay_dur,
                max_delay: max_delay_dur,
                backoff_factor,
                jitter_fraction: jitter,
            };
            retry_cfg
                .validate()
                .map_err(|e| format!("invalid retry config: {}", e))?;
            cmd::fetch::run(&log, &fetch_cfg, &retry_cfg)
        })(),
        Commands::Exec { workdir, args } => cmd::exec::run(&log, &args, &workdir),
    };

    if let Err(e) = result {
        log.error(&e, &[]);
        std::process::exit(1);
    }
}
