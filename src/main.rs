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
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
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
    fn about_command_parses() {
        assert!(matches!(
            Cli::try_parse_from(["biosdevkit", "about"]),
            Ok(Cli {
                command: Command::About
            })
        ));
    }
}
