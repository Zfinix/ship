//! ship: conventional commits in your terminal. Pass the type and summary as
//! flags and it commits what is staged with a conventional message.

mod git;
mod message;

use std::process::ExitCode;

use anyhow::{Result, bail};

use git::GitError;
use message::{Breaking, CommitType, Message};

const NOTHING_STAGED: &str = "Nothing is staged. Stage files with git add, or run ship --all.";
const MAX_GIT_LINES: usize = 6;

const HELP: &str = "ship: conventional commits in your terminal

Usage: ship [options]

Options:
  -t, --type <type>       feat, fix, docs, refactor, perf, test, build, ci,
                          chore, style or revert
  -s, --scope <scope>     the part of the project it touches
  -m, --message <text>    the summary
      --breaking          mark the change as breaking with !
      --all               stage every change first
      --dry-run           print the message instead of committing
  -h, --help              show this help
  -V, --version           show the version";

/// What happens once the message is ready.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Finish {
    /// Commit what is staged.
    Commit,
    /// Print the message and stop.
    DryRun,
}

fn main() -> ExitCode {
    let Err(e) = run() else {
        return ExitCode::SUCCESS;
    };
    eprintln!("{e}");
    if let Some(git) = e.downcast_ref::<GitError>() {
        for row in git.stderr.lines().take(MAX_GIT_LINES) {
            eprintln!("  {row}");
        }
    }
    ExitCode::FAILURE
}

fn run() -> Result<()> {
    let mut kind = None;
    let mut scope = None;
    let mut summary = None;
    let mut breaking = Breaking::No;
    let mut finish = Finish::Commit;
    let mut stage_first = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-t" | "--type" => {
                let name = value(rest.next(), arg)?;
                let Some(parsed) = CommitType::parse(name) else {
                    let all: Vec<&str> = CommitType::ALL.iter().map(|k| k.name()).collect();
                    bail!(
                        "There is no commit type called {name}. Use one of: {}.",
                        all.join(", ")
                    );
                };
                kind = Some(parsed);
            }
            "-s" | "--scope" => scope = Some(value(rest.next(), arg)?.to_string()),
            "-m" | "--message" => summary = Some(value(rest.next(), arg)?.to_string()),
            "--breaking" => breaking = Breaking::Yes,
            "--all" => stage_first = true,
            "--dry-run" => finish = Finish::DryRun,
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("ship {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            other => bail!("There is no {other} option. Run ship --help to see them all."),
        }
    }

    if !git::in_repo() {
        bail!("This folder is not a git repository. Run ship inside one.");
    }
    if stage_first {
        git::stage_all()?;
    }

    let (Some(kind), Some(summary)) = (kind, summary) else {
        bail!("ship needs --type and --message. Run ship --help to see an example.");
    };
    let message =
        Message::new(kind, scope.as_deref(), &summary, breaking).map_err(anyhow::Error::msg)?;
    if finish == Finish::Commit && git::staged()?.is_empty() {
        bail!(NOTHING_STAGED);
    }

    let message = message.to_string();
    match finish {
        Finish::DryRun => println!("{message}"),
        Finish::Commit => {
            let hash = git::commit(&message)?;
            println!("✓ Committed {hash} {message}");
        }
    }
    Ok(())
}

/// The value after `flag`, or a sentence saying it is missing.
fn value<'a>(next: Option<&'a String>, flag: &str) -> Result<&'a str> {
    match next {
        Some(value) => Ok(value),
        None => bail!("{flag} needs a value after it. Run ship --help to see an example."),
    }
}
