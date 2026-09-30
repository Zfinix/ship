//! ship: conventional commits in your terminal. Pass the type and summary as
//! flags and it prints the conventional commit message.

mod message;

use std::process::ExitCode;

use anyhow::{Result, bail};

use message::{Breaking, CommitType, Message};

const HELP: &str = "ship: conventional commits in your terminal

Usage: ship [options]

Options:
  -t, --type <type>       feat, fix, docs, refactor, perf, test, build, ci,
                          chore, style or revert
  -s, --scope <scope>     the part of the project it touches
  -m, --message <text>    the summary
      --breaking          mark the change as breaking with !
  -h, --help              show this help
  -V, --version           show the version";

fn main() -> ExitCode {
    let Err(e) = run() else {
        return ExitCode::SUCCESS;
    };
    eprintln!("{e}");
    ExitCode::FAILURE
}

fn run() -> Result<()> {
    let mut kind = None;
    let mut scope = None;
    let mut summary = None;
    let mut breaking = Breaking::No;

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

    let (Some(kind), Some(summary)) = (kind, summary) else {
        bail!("ship needs --type and --message. Run ship --help to see an example.");
    };
    let message =
        Message::new(kind, scope.as_deref(), &summary, breaking).map_err(anyhow::Error::msg)?;
    println!("{message}");
    Ok(())
}

/// The value after `flag`, or a sentence saying it is missing.
fn value<'a>(next: Option<&'a String>, flag: &str) -> Result<&'a str> {
    match next {
        Some(value) => Ok(value),
        None => bail!("{flag} needs a value after it. Run ship --help to see an example."),
    }
}
