//! `/help` command text builder, extracted from `handle_command`.

/// Return the help lines for the `/help` command.
///
/// If `skill_names` is non-empty, a `/skills` entry is included.
pub fn help_lines(skill_names: &[String]) -> Vec<String> {
    let mut lines = vec![
        "  🤖 AXONIX COMMAND MANIFEST".to_string(),
        String::new(),
        "  📋 Session & Navigation".to_string(),
        "    /help          Show this help".to_string(),
        "    /status        Show session info".to_string(),
        "    /context       Show conversation messages summary".to_string(),
        "    /tokens        Show token usage and cost estimate".to_string(),
        "    /history       Show numbered list of prompts this session".to_string(),
        "    /retry [N]     Retry last prompt, or prompt #N from /history".to_string(),
        "    /clear         Clear conversation history".to_string(),
        "    /model <name>  Switch model (clears history)".to_string(),
        "    /save [path]   Save conversation to file".to_string(),
        "    /goals         Show active goals from GOALS.md".to_string(),
    ];

    if !skill_names.is_empty() {
        lines.push("    /skills        Show loaded skills".to_string());
    }

    lines.push("    /quit, /exit   Exit".to_string());
    lines.push(String::new());

    lines.extend([
        "  💾 Memory & Search".to_string(),
        "    /memory list        Show persistent memory (facts across sessions)".to_string(),
        "    /memory set/get/del Read and write persistent memory".to_string(),
        "    /memory recent      Show 5 most recent semantic memories".to_string(),
        "    /memory-search <q>  Search stored observations by keyword".to_string(),
        "    /search <query>     Semantic similarity search over embeddings store".to_string(),
        String::new(),
        "  🔮 Predictions".to_string(),
        "    /predict add <text> Log a prediction about a future outcome".to_string(),
        "    /predict open       Show open (unresolved) predictions".to_string(),
        "    /predict list       Show all predictions with outcomes".to_string(),
        String::new(),
        "  🐙 GitHub".to_string(),
        "    /issues [N]               List open GitHub issues (default 10, sorted by reactions)".to_string(),
        "    /comment <n> <text>       Post comment on GitHub issue #n".to_string(),
        "    /respond <n> <text>       Post response on GitHub issue #n".to_string(),
        "    /respond <n> close <text> Post response and close issue".to_string(),
        String::new(),
        "  🖥 SSH & System".to_string(),
        "    /ssh list        List registered SSH hosts".to_string(),
        "    /ssh <h> <cmd>   Run command on a remote host".to_string(),
        "    /watch           Show current health vs thresholds".to_string(),
        "    /lint <file>     Validate YAML or Caddyfile syntax".to_string(),
        String::new(),
        "  📊 Reporting & Meta".to_string(),
        "    /brief             Run the morning brief interactively".to_string(),
        "    /summary [text]    Show or update cycle summary".to_string(),
        "    /recap             Post session recap thread to Bluesky".to_string(),
        "    /failures          Show logged failure patterns".to_string(),
        "    /archive-journal   Archive old journal entries".to_string(),
        "    /review <desc>     Invoke code_reviewer sub-agent".to_string(),
        "    /files [N]         List .rs source files over N lines (default 300)".to_string(),
        String::new(),
        "  ⌨  Multiline input".to_string(),
        r#"    End a line with \ to continue on the next line"#.to_string(),
        r#"    Type """ to start a block, """ again to finish"#.to_string(),
        String::new(),
    ]);

    lines
}
