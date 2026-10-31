use anyhow::Result;
use clap::{Parser, Subcommand};
use docx_lint::config::{ProfileStore, default_dir};
use docx_lint::linter;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "docx-lint", about = "Academic .docx linter")]
struct Cli {
    /// Directory holding profile .toml files
    #[arg(long, global = true)]
    profiles_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage university profiles
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Lint a .docx file
    Check {
        file: PathBuf,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Create a profile with default values
    Init { name: String },
    /// Set a dotted key, e.g. `page.top_cm 4.0`
    Set {
        name: String,
        key: String,
        value: String,
    },
    /// Make a profile the active one
    Use { name: String },
    /// Print a profile (default: active)
    Show { name: Option<String> },
}

fn run(cli: Cli) -> Result<bool> {
    let store = ProfileStore::new(cli.profiles_dir.unwrap_or_else(|| default_dir().into()));
    match cli.command {
        Command::Config { action } => {
            match action {
                ConfigAction::Init { name } => {
                    store.init(&name)?;
                    println!("created profile '{name}'");
                }
                ConfigAction::Set { name, key, value } => store.set_value(&name, &key, &value)?,
                ConfigAction::Use { name } => store.set_active(&name)?,
                ConfigAction::Show { name } => {
                    print!(
                        "{}",
                        toml::to_string_pretty(&store.resolve(name.as_deref())?)?
                    )
                }
            }
            Ok(true)
        }
        Command::Check {
            file,
            profile,
            json,
        } => {
            let cfg = store.resolve(profile.as_deref())?;
            let violations = linter::lint_file(&file, &cfg)?;
            if json {
                let doc = serde_json::json!({
                    "file": file.display().to_string(),
                    "profile": cfg.profile_name,
                    "violations": violations,
                });
                println!("{}", serde_json::to_string_pretty(&doc)?);
            } else if violations.is_empty() {
                println!("OK: no violations");
            } else {
                for x in &violations {
                    println!("[{}] (para {}) {}", x.code, x.paragraph, x.message);
                }
            }
            Ok(violations.is_empty())
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
