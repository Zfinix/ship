//! The commit message itself: the eleven types, the first line, and the
//! rules that keep it conventional. Nothing here touches git or the terminal.

use std::fmt;

/// Longest first line ship will write.
pub const HEADER_LIMIT: usize = 72;

/// The kind of change, as the first word of a conventional commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitType {
    Feat,
    Fix,
    Docs,
    Refactor,
    Perf,
    Test,
    Build,
    Ci,
    Chore,
    Style,
    Revert,
}

impl CommitType {
    /// Every type, in the order the picker shows them.
    pub const ALL: [CommitType; 11] = [
        CommitType::Feat,
        CommitType::Fix,
        CommitType::Docs,
        CommitType::Refactor,
        CommitType::Perf,
        CommitType::Test,
        CommitType::Build,
        CommitType::Ci,
        CommitType::Chore,
        CommitType::Style,
        CommitType::Revert,
    ];

    /// The word written into the message.
    pub fn name(self) -> &'static str {
        match self {
            CommitType::Feat => "feat",
            CommitType::Fix => "fix",
            CommitType::Docs => "docs",
            CommitType::Refactor => "refactor",
            CommitType::Perf => "perf",
            CommitType::Test => "test",
            CommitType::Build => "build",
            CommitType::Ci => "ci",
            CommitType::Chore => "chore",
            CommitType::Style => "style",
            CommitType::Revert => "revert",
        }
    }

    /// The type called `name`, ignoring case.
    pub fn parse(name: &str) -> Option<CommitType> {
        let name = name.trim().to_ascii_lowercase();
        CommitType::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// A finished conventional commit message: `type(scope)!: summary`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: CommitType,
    pub scope: Option<String>,
    pub summary: String,
    pub breaking: Breaking,
}

/// Whether the change breaks existing users, marked with `!` after the type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breaking {
    No,
    Yes,
}

impl Message {
    /// Build a message, tidying the scope and summary. Fails with a plain
    /// sentence when the summary is empty or the first line runs past 72.
    pub fn new(
        kind: CommitType,
        scope: Option<&str>,
        summary: &str,
        breaking: Breaking,
    ) -> Result<Message, String> {
        let message = Message {
            kind,
            scope: scope.and_then(normalize_scope),
            summary: normalize_summary(summary),
            breaking,
        };
        if message.summary.is_empty() {
            return Err("The summary is empty. Say in a few words what this commit does.".into());
        }
        let len = message.to_string().chars().count();
        if len > HEADER_LIMIT {
            return Err(format!(
                "The first line is {len} characters. Shorten the summary so it fits in {HEADER_LIMIT}."
            ));
        }
        Ok(message)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind.name())?;
        if let Some(scope) = &self.scope {
            write!(f, "({scope})")?;
        }
        match self.breaking {
            Breaking::Yes => f.write_str("!")?,
            Breaking::No => {}
        }
        write!(f, ": {}", self.summary)
    }
}

/// A scope trimmed, with inner spaces turned into dashes. `None` when empty.
pub fn normalize_scope(scope: &str) -> Option<String> {
    let scope = scope.split_whitespace().collect::<Vec<_>>().join("-");
    (!scope.is_empty()).then_some(scope)
}

/// The summary trimmed, without a trailing period, and starting lowercase
/// unless its first word looks like an acronym or an identifier.
pub fn normalize_summary(summary: &str) -> String {
    let summary = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    let summary = summary.trim_end_matches('.').trim_end();
    let first_word = summary.split(' ').next().unwrap_or_default();
    let keep = first_word.chars().skip(1).any(char::is_uppercase)
        || first_word
            .chars()
            .any(|c| c.is_ascii_digit() || "_:./`()<>#-".contains(c));
    let mut chars = summary.chars();
    match (keep, chars.next()) {
        (false, Some(first)) => first.to_lowercase().chain(chars).collect(),
        (true, _) | (false, None) => summary.to_string(),
    }
}

/// `{"hash":…,"message":…}` for `--json`; `hash` is `null` on a dry run.
pub fn to_json(hash: Option<&str>, message: &str) -> String {
    let quote = |text: &str| {
        let mut out = String::from("\"");
        for c in text.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    };
    let hash = hash.map_or_else(|| "null".to_string(), quote);
    format!("{{\"hash\":{hash},\"message\":{}}}", quote(message))
}

#[cfg(test)]
#[path = "message_test.rs"]
mod tests;
