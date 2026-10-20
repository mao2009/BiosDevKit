use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "biosdevkit",
    version,
    about = "BIOS development and compatibility testing toolkit"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print project status and supported roadmap targets
    About,
    /// Validate Trace Event v1 JSONL from a file or standard input (-).
    ValidateTrace { input: String },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::ValidateTrace { input } => {
            use std::io::{self, BufReader};
            let result = if input == "-" {
                biosdevkit::traces::validate_reader(BufReader::new(io::stdin().lock()))
            } else {
                std::fs::File::open(&input)
                    .map_err(|e| format!("opening {input}: {e}"))
                    .and_then(|f| biosdevkit::traces::validate_reader(BufReader::new(f)))
            };
            match result {
                Ok(count) => println!("validated {count} trace events"),
                Err(error) => {
                    eprintln!("trace validation failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        Command::About => {
            println!("BiosDevKit: early bootstrap; no BIOS compatibility claims.");
            println!("Planned platforms: PCE CD, Mega-CD, Sega Saturn.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_definitions_are_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(Cli::try_parse_from(["biosdevkit", "unknown"]).is_err());
    }

    #[test]
    fn rejects_missing_command() {
        assert!(Cli::try_parse_from(["biosdevkit"]).is_err());
    }

    #[test]
    fn validate_trace_command_parses() {
        assert!(matches!(Cli::try_parse_from(["biosdevkit", "validate-trace", "-"]), Ok(Cli { command: Command::ValidateTrace { .. } })));
    }

    #[test]
    fn about_command_parses() {
        assert!(matches!(
            Cli::try_parse_from(["biosdevkit", "about"]),
            Ok(Cli {
                command: Command::About
            })
        ));
    }
}
