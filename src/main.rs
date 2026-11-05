use anyhow::Result;
use clap::{Parser, Subcommand};
use docx_lint::config::{ProfileStore, default_dir};
use docx_lint::{fixer, linter, report};
use std::io::Write;
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
        /// Write an auto-fixed copy of the document here
        #[arg(long)]
        fix: Option<PathBuf>,
        /// Write an HTML report here
        #[arg(long)]
        html: Option<PathBuf>,
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
    let mut out = std::io::stdout().lock();
    match cli.command {
        Command::Config { action } => {
            match action {
                ConfigAction::Init { name } => {
                    store.init(&name)?;
                    writeln!(out, "created profile '{name}'")?;
                }
                ConfigAction::Set { name, key, value } => store.set_value(&name, &key, &value)?,
                ConfigAction::Use { name } => store.set_active(&name)?,
                ConfigAction::Show { name } => write!(
                    out,
                    "{}",
                    toml::to_string_pretty(&store.resolve(name.as_deref())?)?
                )?,
            }
            Ok(true)
        }
        Command::Check {
            file,
            profile,
            json,
            fix,
            html,
        } => {
            let cfg = store.resolve(profile.as_deref())?;
            let violations = linter::lint_file(&file, &cfg)?;
            if let Some(path) = &html {
                report::write_html(&file, path, &cfg, &violations)?;
            }
            // WHY: with --fix the exit code reflects what is left in the fixed copy.
            let remaining = match &fix {
                Some(out) => {
                    fixer::fix_file(&file, out, &cfg)?;
                    Some(linter::lint_file(out, &cfg)?)
                }
                None => None,
            };
            if json {
                let mut doc = serde_json::json!({
                    "file": file.display().to_string(),
                    "profile": cfg.profile_name,
                    "violations": violations,
                });
                if let (Some(rem), Some(path)) = (&remaining, &fix) {
                    doc["fixed_file"] = path.display().to_string().into();
                    doc["remaining"] = serde_json::to_value(rem)?;
                }
                writeln!(out, "{}", serde_json::to_string_pretty(&doc)?)?;
            } else {
                print_violations(&mut out, &violations)?;
                if let (Some(rem), Some(path)) = (&remaining, &fix) {
                    writeln!(
                        out,
                        "fixed copy: {} ({} remaining)",
                        path.display(),
                        rem.len()
                    )?;
                    print_violations(&mut out, rem)?;
                }
            }
            Ok(remaining.as_ref().unwrap_or(&violations).is_empty())
        }
    }
}

fn print_violations(out: &mut impl Write, violations: &[linter::Violation]) -> std::io::Result<()> {
    if violations.is_empty() {
        writeln!(out, "OK: no violations")?;
    }
    for x in violations {
        writeln!(out, "[{}] (para {}) {}", x.code, x.paragraph, x.message)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    // WHY: 0 args (double-click) means GUI; any args mean headless CLI.
    if std::env::args_os().len() <= 1 {
        return match docx_lint::gui::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        };
    }
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
