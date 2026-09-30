//! The few git commands ship runs, each through `std::process::Command`.

use std::fmt;
use std::process::Command;

/// A git command that failed: a plain sentence for the user and git's own
/// words to show under it.
#[derive(Debug)]
pub struct GitError {
    pub sentence: String,
    pub stderr: String,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.sentence)
    }
}

impl std::error::Error for GitError {}

/// One staged file and its line counts. Binary files count as zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedFile {
    pub path: String,
    pub added: u64,
    pub removed: u64,
}

/// Run git with `args`, returning stdout, or a [`GitError`] carrying
/// `sentence` and git's stderr.
fn git(args: &[&str], sentence: &str) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| GitError {
            sentence: "Could not run git. Install git or put it on your PATH, then try again."
                .into(),
            stderr: e.to_string(),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let raw = match stderr.trim().is_empty() {
            true => stdout.trim().to_string(),
            false => stderr.trim().to_string(),
        };
        return Err(GitError {
            sentence: sentence.to_string(),
            stderr: raw,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// True when the working directory is inside a git work tree.
pub fn in_repo() -> bool {
    git(&["rev-parse", "--is-inside-work-tree"], "").is_ok_and(|out| out.trim() == "true")
}

/// What `git diff --cached` would commit, one entry per file.
pub fn staged() -> Result<Vec<StagedFile>, GitError> {
    let out = git(
        &["diff", "--cached", "--numstat"],
        "Could not read the staged changes. Check the git output below and try again.",
    )?;
    Ok(parse_numstat(&out))
}

/// Stage every change in the work tree, like `git add -A`.
pub fn stage_all() -> Result<(), GitError> {
    git(
        &["add", "-A"],
        "Could not stage your changes. Fix what git says below, then run ship again.",
    )
    .map(drop)
}

/// Commit what is staged with exactly `message` and return the short hash.
pub fn commit(message: &str) -> Result<String, GitError> {
    git(
        &["commit", "--quiet", "--cleanup=verbatim", "-m", message],
        "Git did not make the commit. Fix what it says below, then run ship again.",
    )?;
    let hash = git(
        &["rev-parse", "--short", "HEAD"],
        "The commit was made but its hash could not be read. Run git log to see it.",
    )?;
    Ok(hash.trim().to_string())
}

/// Parse `git diff --numstat` rows: `added<TAB>removed<TAB>path`.
pub fn parse_numstat(out: &str) -> Vec<StagedFile> {
    out.lines()
        .filter_map(|row| {
            let mut fields = row.splitn(3, '\t');
            let added = fields.next()?;
            let removed = fields.next()?;
            let path = fields.next()?;
            Some(StagedFile {
                path: path.to_string(),
                added: added.parse().unwrap_or(0),
                removed: removed.parse().unwrap_or(0),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "git_test.rs"]
mod tests;
