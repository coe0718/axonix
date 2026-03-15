use crate::render::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version() -> &'static str {
    VERSION
}

/// Parsed command-line arguments.
pub struct CliArgs {
    pub model: String,
    pub skill_dirs: Vec<String>,
    pub prompt: Option<String>,
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

        Some(Self {
            model,
            skill_dirs,
            prompt,
        })
    }
}

pub fn print_help() {
    let version = VERSION;
    println!("axonix v{version} — a coding agent growing up in public");
    println!();
    println!("Usage: axonix [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --model <name>    Model to use (default: claude-opus-4-6)");
    println!("  --skills <dir>    Directory containing skill files");
    println!("  -p, --prompt <text>  Run a single prompt and exit (no REPL)");
    println!("  --help, -h        Show this help message");
    println!("  --version, -V     Show version");
    println!();
    println!("Commands (in REPL):");
    println!("  /help             Show available commands");
    println!("  /status           Show session info");
    println!("  /tokens           Show token usage and cost estimate");
    println!("  /quit, /exit      Exit the agent");
    println!("  /clear            Clear conversation history");
    println!("  /retry            Retry the last prompt");
    println!("  /model <name>     Switch model mid-session");
    println!("  /save [path]      Save conversation to file");
    println!();
    println!("Environment:");
    println!("  ANTHROPIC_API_KEY  API key for Anthropic (required)");
    println!("  API_KEY            Alternative env var for API key");
}

pub fn print_banner() {
    let version = VERSION;
    println!(
        "\n{BOLD}{CYAN}  axonix{RESET} v{version} {DIM}— a coding agent growing up in public{RESET}"
    );
    println!("{DIM}  Type /quit to exit, /clear to reset{RESET}\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant_exists() {
        assert!(
            version().contains('.'),
            "Version should contain a dot: {}",
            version()
        );
    }

    #[test]
    fn test_cli_default_model() {
        let args: Vec<String> = vec!["axonix"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.model, "claude-opus-4-6");
        assert!(cli.skill_dirs.is_empty());
        assert!(cli.prompt.is_none());
    }

    #[test]
    fn test_cli_skills_parsing() {
        let args: Vec<String> = vec!["axonix", "--skills", "./my_skills", "--skills", "./more"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.skill_dirs, vec!["./my_skills", "./more"]);
    }

    #[test]
    fn test_cli_help_returns_none() {
        let args: Vec<String> = vec!["axonix", "--help"]
            .into_iter().map(String::from).collect();
        assert!(CliArgs::parse(&args).is_none());
    }

    #[test]
    fn test_cli_version_returns_none() {
        let args: Vec<String> = vec!["axonix", "-V"]
            .into_iter().map(String::from).collect();
        assert!(CliArgs::parse(&args).is_none());
    }

    #[test]
    fn test_prompt_flag_parsing() {
        let args: Vec<String> = vec!["axonix", "-p", "explain monads"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.prompt.as_deref(), Some("explain monads"));
    }

    #[test]
    fn test_prompt_long_flag_parsing() {
        let args: Vec<String> = vec!["axonix", "--prompt", "fix the bug"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.prompt.as_deref(), Some("fix the bug"));
    }

    #[test]
    fn test_prompt_flag_missing_value() {
        let args: Vec<String> = vec!["axonix", "-p"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.prompt.is_none(), "Missing value after -p should yield None");
    }

    #[test]
    fn test_prompt_flag_not_present() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-sonnet-4-20250514"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.prompt.is_none());
        assert_eq!(cli.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_prompt_flag_with_other_flags() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-opus-4-6", "-p", "hello world"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.prompt.as_deref(), Some("hello world"));
        assert_eq!(cli.model, "claude-opus-4-6");
    }

    #[test]
    fn test_command_parsing_model() {
        let input = "/model claude-opus-4-6";
        assert!(input.starts_with("/model "));
        let model_name = input.trim_start_matches("/model ").trim();
        assert_eq!(model_name, "claude-opus-4-6");
    }

    #[test]
    fn test_command_parsing_model_whitespace() {
        let input = "/model   claude-opus-4-6  ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert_eq!(model_name, "claude-opus-4-6");
    }

    #[test]
    fn test_model_command_empty_name() {
        let input = "/model ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert!(model_name.is_empty(), "Empty model name should be detected");
    }

    #[test]
    fn test_model_command_whitespace_only() {
        let input = "/model    ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert!(model_name.is_empty(), "Whitespace-only model name should be detected");
    }
}
