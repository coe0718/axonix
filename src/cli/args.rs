use super::help::print_help;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version() -> &'static str {
    VERSION
}

/// Parsed command-line arguments.
pub struct CliArgs {
    pub model: String,
    pub skill_dirs: Vec<String>,
    pub prompt: Option<String>,
    /// If set, post this text to Bluesky and exit (no agent session started).
    pub bluesky_post: Option<String>,
    /// If set, print the morning brief (open goals, predictions, recent metrics) and exit.
    pub brief: bool,
    /// If set alongside `brief`, also send the brief to Telegram (enables cron-based push).
    pub brief_telegram: bool,
    /// If set, run health watch loop: check thresholds every N seconds and send Telegram alerts.
    pub watch: bool,
    /// If set, run the always-on Telegram listener daemon (polls for /ask commands).
    pub listen: bool,
    /// If set, print system health + Docker container status, alert on unhealthy containers, and exit.
    pub health: bool,
    /// If set, write cycle_summary.json from real data (git log, GOALS.md) and exit.
    pub write_summary: Option<String>,
    /// If set, read .axonix/cycle_summary.json and send a compact summary to Telegram.
    pub session_summary_telegram: bool,
    /// If set, insert this row into METRICS.md using insert_metrics_row() (G-072).
    pub insert_metrics_row: Option<String>,
    /// If set, run memory extraction on the given session log file and exit.
    pub extract_memories: Option<String>,
    /// If set, send this text to Telegram and exit (no agent session started).
    pub telegram_notify: Option<String>,
    /// If set, read stdin line-by-line, redact secrets, and POST each line to this URL.
    /// Used by evolve.sh to stream session output to the dashboard without curl.
    pub stream_pipe: Option<String>,
    /// If true, auto-resolve predictions whose mentioned goal IDs are complete in GOALS_ARCHIVE.md.
    pub predict_auto_resolve: bool,
    /// If true, print a compact health summary (CPU%, mem%, disk%, uptime) and exit. Scriptable.
    pub health_subcommand: bool,
}

impl CliArgs {
    /// Parse CLI arguments. Returns None if --help or --version was handled (program should exit).
    pub fn parse(args: &[String]) -> Option<Self> {
        if args.iter().any(|a| a == "--help" || a == "-h") {
            print_help();
            return None;
        }
        if args.iter().any(|a| a == "--version" || a == "-V") {
            println!("axonix v{VERSION}");
            return None;
        }

        let model = args
            .iter()
            .position(|a| a == "--model")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "claude-opus-4-6".into());

        let skill_dirs: Vec<String> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == "--skills")
            .filter_map(|(i, _)| args.get(i + 1).cloned())
            .collect();

        let prompt = args
            .iter()
            .position(|a| a == "--prompt" || a == "-p")
            .and_then(|i| args.get(i + 1))
            .cloned();

        let bluesky_post = args
            .iter()
            .position(|a| a == "--bluesky-post")
            .and_then(|i| args.get(i + 1))
            .cloned();

        let brief = args.iter().any(|a| a == "--brief");
        let brief_telegram = args.iter().any(|a| a == "--brief-telegram");
        let watch = args.iter().any(|a| a == "--watch");
        let listen = args.iter().any(|a| a == "--listen");
        let health = args.iter().any(|a| a == "--health");
        let session_summary_telegram = args.iter().any(|a| a == "--session-summary-telegram");

        let write_summary = args
            .iter()
            .position(|a| a == "--write-summary")
            .and_then(|i| args.get(i + 1))
            .cloned();

        let insert_metrics_row = args
            .iter()
            .position(|a| a == "--insert-metrics-row" || a == "--insert-row")
            .and_then(|i| args.get(i + 1))
            .cloned();

        let extract_memories = args
            .iter()
            .position(|a| a == "--extract-memories")
            .and_then(|i| args.get(i + 1))
            .cloned();

        let telegram_notify = args
            .iter()
            .position(|a| a == "--telegram-notify")
            .and_then(|i| args.get(i + 1))
            .cloned();

        let stream_pipe = args
            .iter()
            .position(|a| a == "--stream-pipe")
            .and_then(|i| args.get(i + 1))
            .cloned();

        // `predict auto-resolve` or `--predict-auto-resolve`
        let predict_auto_resolve = args.windows(2).any(|w| w[0] == "predict" && w[1] == "auto-resolve")
            || args.iter().any(|a| a == "--predict-auto-resolve");

        // `axonix health` positional subcommand (bare "health" arg, not --health flag)
        let health_subcommand = args.iter().skip(1).any(|a| a.as_str() == "health");

        Some(Self {
            model,
            skill_dirs,
            prompt,
            bluesky_post,
            brief: brief || brief_telegram,
            brief_telegram,
            watch,
            listen,
            health,
            write_summary,
            session_summary_telegram,
            insert_metrics_row,
            extract_memories,
            telegram_notify,
            stream_pipe,
            predict_auto_resolve,
            health_subcommand,
        })
    }
}
