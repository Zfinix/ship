//! The interactive commit: one question at a time in an inline pane, each
//! answer printed into scrollback so the whole walk reads top to bottom.

use std::fmt;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Position;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tokio::sync::mpsc;

use kiln::confirm::Confirm;
use kiln::guard::TuiGuard;
use kiln::input::Input;
use kiln::keys::{self, Binding};
use kiln::list::{ListSelectionView, SelectionItem};
use kiln::render::{Column, Inset, Insets, Renderable};
use kiln::terminal::{Tui, TuiEvent, restore_raw};
use kiln::view::View;
use kiln::{text, theme};

use crate::git;
use crate::message::{
    Breaking, CommitType, Message, normalize_scope, normalize_summary, summary_limit,
};

const MAX_FILES: usize = 8;
const SCOPE_LIMIT: usize = 24;
const PROMPT: &str = "› ";

const CONFIRM_KEYS: [Binding; 4] = [
    ("←→", "move"),
    ("y/n", "answer"),
    ("enter", "confirm"),
    ("esc", "cancel"),
];
const INPUT_KEYS: [Binding; 2] = [("enter", "next"), ("esc", "cancel")];
const SCOPE_KEYS: [Binding; 2] = [("enter", "next, empty for none"), ("esc", "cancel")];

/// Answers already given on the command line; those steps are skipped.
pub struct Preset {
    pub kind: Option<CommitType>,
    pub scope: Option<String>,
    pub summary: Option<String>,
    pub breaking: Option<Breaking>,
}

/// What happens once the message is ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Ask "Commit?" and commit on yes.
    Commit,
    /// Show the message and stop.
    DryRun,
}

/// How the walk ended when it did not fail.
pub enum Outcome {
    /// The message to commit or print.
    Ready(Message),
    /// Nothing was staged and the user chose not to stage anything.
    NothingStaged,
}

/// The user pressed esc or ctrl+c; nothing was committed.
#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Nothing was committed.")
    }
}

impl std::error::Error for Cancelled {}

/// Walk through the commit in an inline pane. Fails with [`Cancelled`] when
/// the user backs out, or with a [`git::GitError`] when git does.
pub async fn run(preset: Preset, finish: Finish) -> Result<Outcome> {
    let _guard = TuiGuard::install(restore_raw);
    let mut tui = Tui::new(3)?;
    let width = tui.width() as usize;
    let t = theme::get();

    let mut files = git::staged()?;
    if files.is_empty() {
        let mut stage = Confirm::new("Nothing is staged. Stage all changes?");
        ask(&mut tui, &mut stage, Vec::new(), &CONFIRM_KEYS).await?;
        if stage.answer() != Some(true) {
            return Ok(Outcome::NothingStaged);
        }
        git::stage_all()?;
        files = git::staged()?;
        if files.is_empty() {
            return Ok(Outcome::NothingStaged);
        }
    }

    let added: u64 = files.iter().map(|f| f.added).sum();
    let removed: u64 = files.iter().map(|f| f.removed).sum();
    let mut block = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Staged  ", t.bold_style()),
            Span::styled(text::count_of(files.len(), "file"), t.dim_style()),
            Span::styled(format!("  +{added}"), Style::default().fg(t.add_fg)),
            Span::styled(format!(" -{removed}"), Style::default().fg(t.del_fg)),
        ]),
    ];
    let longest = files.iter().map(|f| f.path.chars().count()).max();
    let path_w = longest.unwrap_or(0).min(width.saturating_sub(24).max(12));
    for file in files.iter().take(MAX_FILES) {
        block.push(Line::from(vec![
            Span::styled(
                format!("    {:<path_w$}", text::clip_row(&file.path, path_w)),
                t.dimmer_style(),
            ),
            Span::styled(format!("  +{}", file.added), Style::default().fg(t.add_fg)),
            Span::styled(format!(" -{}", file.removed), Style::default().fg(t.del_fg)),
        ]));
    }
    if files.len() > MAX_FILES {
        block.push(Line::from(Span::styled(
            format!("    +{} more", files.len() - MAX_FILES),
            t.faint_style(),
        )));
    }
    block.push(Line::from(""));
    tui.insert_history(block)?;

    let kind = match preset.kind {
        Some(kind) => kind,
        None => {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let items = CommitType::ALL
                .into_iter()
                .map(|kind| SelectionItem {
                    name: kind.name().to_string(),
                    description: kind.description().to_string(),
                    is_current: false,
                    event: kind,
                })
                .collect();
            let mut list = ListSelectionView::new("What kind of change is this?", items, tx, None);
            ask(&mut tui, &mut list, Vec::new(), &[]).await?;
            rx.try_recv().map_err(|_| Cancelled)?
        }
    };
    tui.insert_history(answered("Type", kind.name()))?;

    let scope = match preset.scope {
        Some(scope) => scope,
        None => {
            let mut input = Input::new(PROMPT).limit(SCOPE_LIMIT);
            let question = question("Scope, the part of the project it touches (optional)");
            ask(&mut tui, &mut input, question, &SCOPE_KEYS).await?;
            input.text().to_string()
        }
    };
    let scope = normalize_scope(&scope);
    tui.insert_history(answered("Scope", scope.as_deref().unwrap_or("none")))?;

    let summary = match preset.summary {
        Some(summary) => normalize_summary(&summary),
        None => {
            let limit = summary_limit(kind, scope.as_deref());
            let mut prompt = "Summary, what the commit does, like \u{201c}add dark mode\u{201d}";
            loop {
                let mut input = Input::new(PROMPT).limit(limit);
                ask(&mut tui, &mut input, question(prompt), &INPUT_KEYS).await?;
                let summary = normalize_summary(input.text());
                if !summary.is_empty() {
                    break summary;
                }
                prompt = "The summary can't be empty. Say what the commit does";
            }
        }
    };
    tui.insert_history(answered("Summary", &summary))?;

    let breaking = match preset.breaking {
        Some(breaking) => breaking,
        None => {
            let mut confirm = Confirm::new("Breaking change?");
            ask(&mut tui, &mut confirm, Vec::new(), &CONFIRM_KEYS).await?;
            match confirm.answer() {
                Some(true) => Breaking::Yes,
                Some(false) | None => Breaking::No,
            }
        }
    };
    let breaking_label = match breaking {
        Breaking::Yes => "yes",
        Breaking::No => "no",
    };
    tui.insert_history(answered("Breaking", breaking_label))?;

    let message =
        Message::new(kind, scope.as_deref(), &summary, breaking).map_err(anyhow::Error::msg)?;
    match finish {
        Finish::DryRun => return Ok(Outcome::Ready(message)),
        Finish::Commit => {}
    }
    let preview = vec![
        Line::from(Span::styled("Your commit message", t.bold_style())),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {message}"),
            t.accent_style().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    let mut confirm = Confirm::new("Commit?").default_yes();
    ask(&mut tui, &mut confirm, preview, &CONFIRM_KEYS).await?;
    match confirm.answer() {
        Some(true) => Ok(Outcome::Ready(message)),
        Some(false) | None => Err(Cancelled.into()),
    }
}

/// Show `view` under `above` until it completes. Esc and ctrl+c cancel the
/// whole walk rather than just this step.
async fn ask<V: View>(
    tui: &mut Tui,
    view: &mut V,
    above: Vec<Line<'static>>,
    bindings: &[Binding],
) -> Result<()> {
    let footer = match bindings.is_empty() {
        true => Vec::new(),
        false => vec![Line::from(""), keys::footer(bindings)],
    };
    while !view.is_complete() {
        let mut column = Column::new();
        column.push(&above);
        column.push(&*view);
        column.push(&footer);
        let pane = Inset::new(column, Insets::tlbr(1, 2, 0, 2));
        tui.draw(pane.desired_height(tui.width()), |frame| {
            let area = frame.area();
            pane.render(area, frame.buffer_mut());
            if let Some((x, y)) = pane.cursor_pos(area) {
                frame.set_cursor_position(Position::new(x, y));
            }
        })?;
        drop(pane);
        match tui.next_event().await {
            TuiEvent::Key(key) => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Esc => return Err(Cancelled.into()),
                    KeyCode::Char('c') if ctrl => return Err(Cancelled.into()),
                    _ => view.handle_key(key),
                }
            }
            TuiEvent::Paste(text) => {
                view.handle_paste(text);
            }
            TuiEvent::Resize => tui.resized()?,
            TuiEvent::Mouse(_) | TuiEvent::Draw => {}
        }
    }
    Ok(())
}

/// A bold question with a blank row under it.
fn question(text: &str) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(text.to_string(), theme::get().bold_style())),
        Line::from(""),
    ]
}

/// A finished step as it stays in scrollback: `✓ Label  value`.
fn answered(label: &str, value: &str) -> Vec<Line<'static>> {
    let t = theme::get();
    vec![Line::from(vec![
        Span::styled("  ✓ ", t.accent_style()),
        Span::styled(format!("{label:<9}"), t.dim_style()),
        Span::styled(value.to_string(), t.text_style()),
    ])]
}
