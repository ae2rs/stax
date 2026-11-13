//! Draws the dashboard: section tabs, the PR table with its stack trees, the preview and a footer.

use std::time::SystemTime;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Padding, Paragraph, Row, Table, TableState, Tabs, Wrap,
};

use crate::app::{App, SectionView};
use crate::github::{Checks, PullRequest, Review};

const REPO_WIDTH: u16 = 20;
const AUTHOR_WIDTH: u16 = 14;

pub fn draw(frame: &mut Frame, app: &App) {
    let [tabs, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_tabs(frame, tabs, app);
    if app.preview() {
        let [table, preview] =
            Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                .areas(body);
        draw_table(frame, table, app);
        draw_preview(frame, preview, app);
    } else {
        draw_table(frame, body, app);
    }
    draw_footer(frame, footer, app);
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles = app.sections().iter().map(|view| {
        let count = if view.loading && view.stacks.is_empty() {
            "…".to_owned()
        } else {
            view.stacks.len().to_string()
        };
        let busy = if view.loading { "↻" } else { " " };
        format!("{} {count}{busy}", view.section.title)
    });
    let tabs = Tabs::new(titles)
        .select(app.tab())
        .style(Style::new().dim())
        .highlight_style(Style::new().not_dim().bold().fg(Color::Yellow));
    frame.render_widget(tabs, area);
}

fn draw_table(frame: &mut Frame, area: Rect, app: &App) {
    let view = app.current();
    if let Some(message) = placeholder(view) {
        frame.render_widget(
            Paragraph::new(message).wrap(Wrap { trim: true }),
            area.inner(Margin::new(1, 1)),
        );
        return;
    }
    let rows: Vec<Row> = app
        .rows()
        .into_iter()
        .map(|row| {
            let pr = view.stacks.pr(row.index);
            let marker = match (row.has_children, row.hidden > 0) {
                (false, _) => "  ",
                (true, false) => "▾ ",
                (true, true) => "▸ ",
            };
            let title_style = if pr.draft {
                Style::new().dim()
            } else {
                Style::new()
            };
            let mut title = vec![
                Span::raw(row.guide).dim(),
                Span::raw(marker).fg(Color::Yellow),
                Span::raw(format!("#{} ", pr.number)).fg(Color::Cyan),
            ];
            // Before the title, which is the part that gets cut.
            if row.hidden > 0 {
                title.push(Span::raw(format!("+{} ", row.hidden)).fg(Color::Yellow));
            }
            title.push(Span::styled(pr.title.clone(), title_style));
            Row::new([
                Cell::from(Line::from(title)),
                Cell::from(pr.repo.rsplit('/').next().unwrap_or(&pr.repo).to_owned()).dim(),
                Cell::from(format!("@{}", pr.author)).dim(),
                Cell::from(review_icon(pr.review)),
                Cell::from(checks_icon(pr.checks)),
                Cell::from(Line::from(vec![
                    Span::raw(format!("+{}", pr.additions)).fg(Color::Green),
                    Span::raw(" "),
                    Span::raw(format!("-{}", pr.deletions)).fg(Color::Red),
                ])),
                Cell::from(age(pr.updated_at)).dim(),
            ])
        })
        .collect();
    let header = Row::new(["PR", "Repo", "Author", "R", "C", "Lines", "Age"])
        .bold()
        .dim();
    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(REPO_WIDTH),
            Constraint::Length(AUTHOR_WIDTH),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(13),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .row_highlight_style(Style::new().bg(Color::DarkGray));
    let mut state = TableState::default().with_selected(Some(app.selected()));
    frame.render_stateful_widget(table, area, &mut state);
}

/// What the table area says instead of rows, if anything.
fn placeholder(view: &SectionView) -> Option<String> {
    if let Some(error) = &view.error {
        return Some(format!("Cannot load {}: {error}", view.section.title));
    }
    if !view.stacks.is_empty() {
        return None;
    }
    Some(if view.loading {
        "Loading…".to_owned()
    } else {
        "No pull requests.".to_owned()
    })
}

fn draw_preview(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::new()
        .borders(Borders::LEFT)
        .border_style(Style::new().dim());
    let Some(pr) = app.selected_pr() else {
        frame.render_widget(block, area);
        return;
    };
    let mut lines = vec![
        Line::from(pr.title.clone()).bold(),
        Line::from(format!("{} #{} · @{}", pr.repo, pr.number, pr.author)).dim(),
        Line::from(vec![
            Span::raw(pr.head.clone()).fg(Color::Cyan),
            Span::raw(" → ").dim(),
            Span::raw(pr.base.clone()).fg(Color::Cyan),
        ]),
        status_line(pr),
        Line::from(pr.url.clone()).dim(),
        Line::default(),
    ];
    lines.extend(pr.body.lines().map(|line| Line::from(line.to_owned())));
    let preview = Paragraph::new(lines)
        .block(block.padding(Padding::horizontal(1)))
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll(), 0));
    frame.render_widget(preview, area);
}

fn status_line(pr: &PullRequest) -> Line<'static> {
    let mut spans = Vec::new();
    if pr.draft {
        spans.push(Span::raw("draft · ").dim());
    }
    let review = match pr.review {
        Review::Approved => Some(Span::raw("approved").fg(Color::Green)),
        Review::ChangesRequested => Some(Span::raw("changes requested").fg(Color::Red)),
        Review::Required => Some(Span::raw("review required").fg(Color::Yellow)),
        Review::None => None,
    };
    let checks = match pr.checks {
        Checks::Success => Some(Span::raw("checks pass").fg(Color::Green)),
        Checks::Failure => Some(Span::raw("checks fail").fg(Color::Red)),
        Checks::Pending => Some(Span::raw("checks running").fg(Color::Yellow)),
        Checks::None => None,
    };
    for span in [review, checks].into_iter().flatten() {
        spans.push(span);
        spans.push(Span::raw(" · ").dim());
    }
    spans.push(Span::raw(format!("+{}", pr.additions)).fg(Color::Green));
    spans.push(Span::raw(format!(" -{}", pr.deletions)).fg(Color::Red));
    spans.push(Span::raw(format!(" · updated {} ago", age(pr.updated_at))).dim());
    Line::from(spans)
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let line = match app.status() {
        Some(status) => Line::from(status).fg(Color::Yellow),
        None => {
            let mut hints = String::from(
                "j/k move · h/l section · space fold · p preview · o open · y copy · d diff · C checkout · v approve · m merge · r/R refresh · q quit",
            );
            for binding in app.keybindings() {
                hints.push_str(&format!(" · {} {}", binding.key, binding.name));
            }
            Line::from(hints).dim()
        }
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn review_icon(review: Review) -> Span<'static> {
    match review {
        Review::Approved => Span::raw("✓").fg(Color::Green),
        Review::ChangesRequested => Span::raw("✗").fg(Color::Red),
        Review::Required => Span::raw("•").fg(Color::Yellow),
        Review::None => Span::raw(" "),
    }
}

fn checks_icon(checks: Checks) -> Span<'static> {
    match checks {
        Checks::Success => Span::raw("●").fg(Color::Green),
        Checks::Failure => Span::raw("●").fg(Color::Red),
        Checks::Pending => Span::raw("●").fg(Color::Yellow),
        Checks::None => Span::raw(" "),
    }
}

/// Time since `then` in its largest whole unit: `45s`, `3h`, `2w`.
fn age(then: SystemTime) -> String {
    let seconds = SystemTime::now()
        .duration_since(then)
        .map_or(0, |age| age.as_secs());
    let (value, unit) = match seconds {
        s if s < 60 => (s, "s"),
        s if s < 3_600 => (s / 60, "m"),
        s if s < 86_400 => (s / 3_600, "h"),
        s if s < 7 * 86_400 => (s / 86_400, "d"),
        s if s < 365 * 86_400 => (s / (7 * 86_400), "w"),
        s => (s / (365 * 86_400), "y"),
    };
    format!("{value}{unit}")
}
