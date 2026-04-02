#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parse_plan_command() {
        let args = vec!["telisq", "plan"];
        let cli = Cli::parse_from(args);
        
        assert!(matches!(cli.command, Commands::Plan(_)));
    }

    #[test]
    fn test_cli_parse_run_command() {
        let args = vec!["telisq", "run"];
        let cli = Cli::parse_from(args);
        
        assert!(matches!(cli.command, Commands::Run(_)));
    }

    #[test]
    fn test_cli_parse_index_command() {
        let args = vec!["telisq", "index"];
        let cli = Cli::parse_from(args);
        
        assert!(matches!(cli.command, Commands::Index(_)));
    }

    #[test]
    fn test_cli_parse_status_command() {
        let args = vec!["telisq", "status"];
        let cli = Cli::parse_from(args);
        
        assert!(matches!(cli.command, Commands::Status(_)));
    }

    #[test]
    fn test_cli_parse_session_command() {
        let args = vec!["telisq", "session", "list"];
        let cli = Cli::parse_from(args);
        
        assert!(matches!(cli.command, Commands::Session(_)));
    }

    #[test]
    fn test_cli_parse_doctor_command() {
        let args = vec!["telisq", "doctor"];
        let cli = Cli::parse_from(args);
        
        assert!(matches!(cli.command, Commands::Doctor(_)));
    }

    #[test]
    fn test_cli_parse_bootstrap_command() {
        let args = vec!["telisq", "bootstrap"];
        let cli = Cli::parse_from(args);
        
        assert!(matches!(cli.command, Commands::Bootstrap));
    }
}
