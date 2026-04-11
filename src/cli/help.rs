use crate::render::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn print_help() {
    let version = VERSION;
    println!("axonix v{version} — a coding agent growing up in public");
    println!();
    println!("Usage: axonix [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --model <name>          Model to use (default: claude-opus-4-6)");
    println!("  --skills <dir>          Directory containing skill files");
    println!("  -p, --prompt <text>     Run a single prompt and exit (no REPL)");
    println!("  --bluesky-post <text>   Post to Bluesky and exit (requires BLUESKY_IDENTIFIER + BLUESKY_APP_PASSWORD)");
    println!("  --brief                 Print morning brief (goals, predictions, metrics) and exit");
    println!("  --brief-telegram        Send morning brief to Telegram and exit (for cron push at 7 AM)");
    println!("  --watch                 Start health watch loop: alert via Telegram when thresholds exceeded");
    println!("  --health                Print system health + Docker container status; alert on unhealthy containers");
    println!("  --listen                Run always-on Telegram listener daemon (polls for /ask commands)");
    println!();
    println!("Subcommands:");
    println!("  health                  Print CPU%, mem%, disk%, uptime and exit (scriptable; no Docker/Telegram)");
    println!("  --write-summary <label> Write .axonix/cycle_summary.json from real git/GOALS data and exit");
    println!("  --session-summary-telegram  Read .axonix/cycle_summary.json and send a compact summary to Telegram");
    println!("  --insert-metrics-row <row>  Insert a row into METRICS.md (ordered, deduplicated)");
    println!("  --extract-memories <path>   Extract memories from session log and store in DB, then exit");
    println!("  --help, -h              Show this help message");
    println!("  --version, -V           Show version");
    println!();
    println!("Commands (in REPL):");
    println!("  /help             Show available commands");
    println!("  /status           Show session info (model, tokens, messages, elapsed)");
    println!("  /context          Show conversation messages summary");
    println!("  /tokens           Show token usage and cost estimate");
    println!("  /history          Show numbered list of prompts this session");
    println!("  /retry [N]        Retry last prompt, or prompt #N from /history");
    println!("  /clear            Clear conversation history");
    println!("  /model <name>     Switch model mid-session (clears history)");
    println!("  /save [path]      Save conversation to file (default: conversation.md)");
    println!("  /lint <file>      Validate YAML or Caddyfile syntax");
    println!("  /ssh list         List registered SSH hosts");
    println!("  /ssh <h> <cmd>    Run a command on a remote host via SSH");
    println!("  /skills           Show loaded skills (when --skills is set)");
    println!("  /quit, /exit      Exit the agent");
    println!();
    println!("Multiline input:");
    println!(r#"  End a line with \ to continue on the next line"#);
    println!(r#"  Type """ to start a block, """ again to finish"#);
    println!();
    println!("Environment:");
    println!("  ANTHROPIC_API_KEY  API key for Anthropic (required)");
    println!("  API_KEY            Alternative env var for API key");
}

pub fn print_banner() {
    let version = VERSION;
    println!(
        "\n{BOLD}{CYAN}  ⚡ AXONIX ONLINE{RESET} v{version} {DIM}:: autonomous coding agent :: evolving in public{RESET}"
    );
    println!("{DIM}  🔧 systems nominal — awaiting input — type /help for command manifest{RESET}\n");
}

/// Print the startup banner in brief/cron mode (no color, machine-readable).
pub fn print_brief_banner() {
    let version = VERSION;
    println!("[ AXONIX v{version} — MORNING BRIEF ]");
}
