//! ship: conventional commits in your terminal. Bare `ship` walks through the
//! commit one question at a time; flags and stdin answer them up front.

mod flow;
mod git;
mod message;

use std::io::{self, IsTerminal, Read};
use std::process::ExitCode;

use anyhow::{Result, bail};
use ratatui::text::{Line, Span};

use kiln::{text, theme};

use flow::{Cancelled, Finish, Outcome, Preset};
use git::GitError;
use message::{Breaking, CommitType, Message, to_json};

const NOTHING_STAGED: &str = "Nothing is staged. Stage files with git add, or run ship --all.";
const MAX_GIT_LINES: usize = 6;

const HELP: &str = "ship: conventional commits in your terminal

Usage: ship [options]

Run it with no options to be asked each part of the message.

Options:
  -t, --type <type>       feat, fix, docs, refactor, perf, test, build, ci,
                          chore, style or revert
  -s, --scope <scope>     the part of the project it touches
  -m, --message <text>    the summary; read from stdin when piped
      --breaking          mark the change as breaking with !
      --all               stage every change first
      --dry-run           print the message instead of committing
  -q, --quiet             print only errors
      --json              print {\"hash\",\"message\"} after committing
      --theme <name>      colour theme, orchid by default (or SHIP_THEME)
      --themes            list the themes
  -h, --help              show this help
  -V, --version           show the version

With both --type and --message, ship commits without asking anything.";

/// How much ship says once it is done.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    Normal,
    Quiet,
    Json,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let Err(e) = run().await else {
        return ExitCode::SUCCESS;
    };
    let t = theme::get();
    let mut lines = Vec::new();
    match (e.downcast_ref::<Cancelled>(), e.downcast_ref::<GitError>()) {
        (Some(cancelled), _) => {
            lines.push(Line::from(Span::styled(
                cancelled.to_string(),
                t.dim_style(),
            )));
        }
        (None, Some(git)) => {
            lines.push(Line::from(Span::styled(
                git.sentence.clone(),
                t.error_style(),
            )));
            for row in git.stderr.lines().take(MAX_GIT_LINES) {
                lines.push(Line::from(Span::styled(
                    format!("  {row}"),
                    t.dimmer_style(),
                )));
            }
        }
        (None, None) => lines.push(Line::from(Span::styled(e.to_string(), t.error_style()))),
    }
    eprint!("{}", paint(&lines, &io::stderr()));
    ExitCode::FAILURE
}

async fn run() -> Result<()> {
    let mut preset = Preset {
        kind: None,
        scope: None,
        summary: None,
        breaking: None,
    };
    let mut finish = Finish::Commit;
    let mut stage_first = false;
    let mut output = Output::Normal;
    let mut theme_name = std::env::var("SHIP_THEME").unwrap_or_else(|_| "orchid".into());
    let names = || {
        theme::all()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>()
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-t" | "--type" => {
                let name = value(rest.next(), arg)?;
                let Some(kind) = CommitType::parse(name) else {
                    let all: Vec<&str> = CommitType::ALL.iter().map(|k| k.name()).collect();
                    bail!(
                        "There is no commit type called {name}. Use one of: {}.",
                        all.join(", ")
                    );
                };
                preset.kind = Some(kind);
            }
            "-s" | "--scope" => preset.scope = Some(value(rest.next(), arg)?.to_string()),
            "-m" | "--message" => preset.summary = Some(value(rest.next(), arg)?.to_string()),
            "--breaking" => preset.breaking = Some(Breaking::Yes),
            "--all" => stage_first = true,
            "--dry-run" => finish = Finish::DryRun,
            "-q" | "--quiet" => output = Output::Quiet,
            "--json" => output = Output::Json,
            "--theme" => theme_name = value(rest.next(), arg)?.to_string(),
            "--themes" => {
                println!("{}", names().join("\n"));
                return Ok(());
            }
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

    let Some(entry) = theme::named(&theme_name) else {
        bail!(
            "There is no theme called {theme_name}. Try one of: {}.",
            names().join(", ")
        );
    };
    theme::set(entry.theme);
    theme::settle();

    if !git::in_repo() {
        bail!("This folder is not a git repository. Run ship inside one.");
    }
    if stage_first {
        git::stage_all()?;
    }

    let stdin_tty = io::stdin().is_terminal();
    if preset.summary.is_none() && !stdin_tty {
        let mut piped = String::new();
        io::stdin().read_to_string(&mut piped)?;
        let piped = piped.trim();
        if !piped.is_empty() {
            preset.summary = Some(piped.to_string());
        }
    }

    let message = match (preset.kind, preset.summary.as_deref()) {
        (Some(kind), Some(summary)) => {
            let breaking = preset.breaking.unwrap_or(Breaking::No);
            let message = Message::new(kind, preset.scope.as_deref(), summary, breaking)
                .map_err(anyhow::Error::msg)?;
            if finish == Finish::Commit && git::staged()?.is_empty() {
                bail!(NOTHING_STAGED);
            }
            message
        }
        (None, _) | (_, None) => {
            if !stdin_tty || !io::stdout().is_terminal() {
                bail!(
                    "ship needs a terminal to ask its questions. Pass --type and --message to commit without one."
                );
            }
            match flow::run(preset, finish).await? {
                Outcome::Ready(message) => message,
                Outcome::NothingStaged => bail!(NOTHING_STAGED),
            }
        }
    };

    let message = message.to_string();
    let hash = match finish {
        Finish::Commit => Some(git::commit(&message)?),
        Finish::DryRun => None,
    };
    let t = theme::get();
    match (output, hash) {
        (Output::Quiet, _) => {}
        (Output::Json, hash) => println!("{}", to_json(hash.as_deref(), &message)),
        (Output::Normal, None) => println!("{message}"),
        (Output::Normal, Some(hash)) => {
            let line = Line::from(vec![
                Span::styled("✓ ", t.accent_style()),
                Span::styled("Committed ", t.dim_style()),
                Span::styled(format!("{hash} "), t.amber_style()),
                Span::styled(message, t.text_style()),
            ]);
            print!("{}", paint(&[line], &io::stdout()));
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

/// Lines as text for `stream`: themed escapes on a terminal, plain otherwise.
fn paint(lines: &[Line<'static>], stream: &impl IsTerminal) -> String {
    if stream.is_terminal() {
        return text::to_ansi(lines);
    }
    lines.iter().map(|line| format!("{line}\n")).collect()
}
