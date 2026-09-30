//! ship: conventional commits in your terminal.

use std::process::ExitCode;

use anyhow::{Result, bail};

const HELP: &str = "ship: conventional commits in your terminal

Usage: ship [options]

Options:
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
    let Some(arg) = std::env::args().nth(1) else {
        println!("{HELP}");
        return Ok(());
    };
    match arg.as_str() {
        "-h" | "--help" => println!("{HELP}"),
        "-V" | "--version" => println!("ship {}", env!("CARGO_PKG_VERSION")),
        other => bail!("There is no {other} option. Run ship --help to see them all."),
    }
    Ok(())
}
