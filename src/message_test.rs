use super::*;

fn message(kind: CommitType, scope: Option<&str>, summary: &str, breaking: Breaking) -> String {
    match Message::new(kind, scope, summary, breaking) {
        Ok(message) => message.to_string(),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn format_without_scope() {
    assert_eq!(
        message(CommitType::Fix, None, "fix the resize bug", Breaking::No),
        "fix: fix the resize bug"
    );
}

#[test]
fn format_with_scope() {
    assert_eq!(
        message(CommitType::Feat, Some("cli"), "add the thing", Breaking::No),
        "feat(cli): add the thing"
    );
}

#[test]
fn format_breaking() {
    assert_eq!(
        message(CommitType::Refactor, Some("api"), "drop v1", Breaking::Yes),
        "refactor(api)!: drop v1"
    );
}

#[test]
fn format_breaking_without_scope() {
    assert_eq!(
        message(CommitType::Feat, None, "drop v1", Breaking::Yes),
        "feat!: drop v1"
    );
}

#[test]
fn format_blank_scope_is_no_scope() {
    assert_eq!(
        message(CommitType::Docs, Some("  "), "tidy", Breaking::No),
        "docs: tidy"
    );
}

#[test]
fn format_rejects_empty_summary() {
    assert_eq!(
        Message::new(CommitType::Fix, None, " . ", Breaking::No),
        Err("The summary is empty. Say in a few words what this commit does.".to_string())
    );
}

#[test]
fn format_rejects_long_first_line() {
    let summary = "a".repeat(70);
    assert_eq!(
        Message::new(CommitType::Feat, None, &summary, Breaking::No),
        Err("The first line is 76 characters. Shorten the summary so it fits in 72.".to_string())
    );
}

#[test]
fn summary_strips_trailing_period() {
    assert_eq!(normalize_summary("add the thing."), "add the thing");
    assert_eq!(normalize_summary("add the thing..."), "add the thing");
}

#[test]
fn summary_lowercases_first_letter() {
    assert_eq!(normalize_summary("Add the thing"), "add the thing");
}

#[test]
fn summary_keeps_acronyms_and_identifiers() {
    assert_eq!(normalize_summary("README tweaks"), "README tweaks");
    assert_eq!(normalize_summary("JSON output"), "JSON output");
    assert_eq!(
        normalize_summary("InlineTerm resizes"),
        "InlineTerm resizes"
    );
    assert_eq!(normalize_summary("Cargo.toml bump"), "Cargo.toml bump");
    assert_eq!(normalize_summary("MAX_WIDTH to 240"), "MAX_WIDTH to 240");
}

#[test]
fn summary_collapses_whitespace() {
    assert_eq!(normalize_summary("  add   the\tthing "), "add the thing");
}

#[test]
fn summary_limit_leaves_room_for_prefix_and_bang() {
    assert_eq!(summary_limit(CommitType::Feat, None), 72 - "feat!: ".len());
    assert_eq!(
        summary_limit(CommitType::Refactor, Some("cli")),
        72 - "refactor(cli)!: ".len()
    );
}

#[test]
fn scope_normalize_joins_words() {
    assert_eq!(normalize_scope(" the cli "), Some("the-cli".to_string()));
    assert_eq!(normalize_scope(""), None);
}

#[test]
fn type_parse_ignores_case() {
    assert_eq!(CommitType::parse("FEAT"), Some(CommitType::Feat));
    assert_eq!(CommitType::parse("feature"), None);
}

#[test]
fn json_escapes_quotes() {
    assert_eq!(
        to_json(Some("abc1234"), "fix: say \"hi\""),
        r#"{"hash":"abc1234","message":"fix: say \"hi\""}"#
    );
}

#[test]
fn json_dry_run_has_null_hash() {
    assert_eq!(
        to_json(None, "docs: tidy"),
        r#"{"hash":null,"message":"docs: tidy"}"#
    );
}
