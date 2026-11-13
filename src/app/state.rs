use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::{Config, Keybinding, Section};
use crate::github::PullRequest;
use crate::stack::{Row, Stacks};

/// The dashboard's state. Keys change it and return the [`Effect`] the event loop carries out.
pub struct App {
    sections: Vec<SectionView>,
    tab: usize,
    /// Node ids of folded PRs, kept across refreshes and sections.
    folded: HashSet<String>,
    keybindings: Vec<Keybinding>,
    preview: bool,
    preview_scroll: u16,
    status: Option<String>,
    confirm: Option<Confirm>,
}

pub struct SectionView {
    pub section: Section,
    pub stacks: Stacks,
    pub loading: bool,
    pub error: Option<String>,
    /// Row index into the visible rows.
    selected: usize,
}

/// Something the event loop does outside the state.
#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Quit,
    /// Fetch these sections, by index.
    Fetch(Vec<usize>),
    /// Suspend the dashboard and run a shell command in the terminal; refetch the current
    /// section afterwards when `refresh` is set.
    Shell {
        command: String,
        refresh: bool,
    },
    OpenUrl(String),
    Copy(String),
    Approve(Box<PullRequest>),
}

/// An action waiting for `y`.
enum Confirm {
    Approve(Box<PullRequest>),
}

impl App {
    pub fn new(config: Config) -> Self {
        let sections = config
            .sections
            .into_iter()
            .map(|section| SectionView {
                section,
                stacks: Stacks::new(Vec::new()),
                loading: false,
                error: None,
                selected: 0,
            })
            .collect();
        App {
            sections,
            tab: 0,
            folded: HashSet::new(),
            keybindings: config.keybindings,
            preview: true,
            preview_scroll: 0,
            status: None,
            confirm: None,
        }
    }

    pub fn sections(&self) -> &[SectionView] {
        &self.sections
    }

    pub fn tab(&self) -> usize {
        self.tab
    }

    pub fn current(&self) -> &SectionView {
        &self.sections[self.tab]
    }

    pub fn rows(&self) -> Vec<Row> {
        self.current().stacks.rows(&self.folded)
    }

    pub fn selected(&self) -> usize {
        self.current().selected
    }

    pub fn selected_pr(&self) -> Option<&PullRequest> {
        let section = self.current();
        let row = self.rows().into_iter().nth(section.selected)?;
        Some(section.stacks.pr(row.index))
    }

    pub fn preview(&self) -> bool {
        self.preview
    }

    pub fn preview_scroll(&self) -> u16 {
        self.preview_scroll
    }

    pub fn keybindings(&self) -> &[Keybinding] {
        &self.keybindings
    }

    /// The line the footer shows instead of key hints: a confirmation prompt or the last status.
    pub fn status(&self) -> Option<String> {
        match &self.confirm {
            Some(Confirm::Approve(pr)) => Some(format!("Approve #{} {}? y/n", pr.number, pr.title)),
            None => self.status.clone(),
        }
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(status.into());
    }

    pub fn fetch_all(&mut self) -> Effect {
        self.fetch((0..self.sections.len()).collect())
    }

    pub fn fetch_current(&mut self) -> Effect {
        self.fetch(vec![self.tab])
    }

    fn fetch(&mut self, indices: Vec<usize>) -> Effect {
        for &i in &indices {
            self.sections[i].loading = true;
        }
        Effect::Fetch(indices)
    }

    pub fn fetched(&mut self, index: usize, result: anyhow::Result<Vec<PullRequest>>) {
        let view = &mut self.sections[index];
        let selected_id = view
            .stacks
            .rows(&self.folded)
            .get(view.selected)
            .map(|row| view.stacks.pr(row.index).id.clone());
        view.loading = false;
        match result {
            Ok(prs) => {
                view.error = None;
                view.stacks = Stacks::new(prs);
            }
            Err(err) => view.error = Some(format!("{err:#}")),
        }
        let rows = view.stacks.rows(&self.folded);
        let kept = selected_id.and_then(|id| {
            rows.iter()
                .position(|row| view.stacks.pr(row.index).id == id)
        });
        view.selected = kept
            .unwrap_or(view.selected)
            .min(rows.len().saturating_sub(1));
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Effect> {
        if let Some(confirm) = self.confirm.take() {
            return match (key.code, confirm) {
                (KeyCode::Char('y'), Confirm::Approve(pr)) => Some(Effect::Approve(pr)),
                _ => None,
            };
        }
        self.status = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(Effect::Quit);
        }
        if let Some(binding) = self
            .keybindings
            .iter()
            .find(|binding| binding.key.matches(&key))
        {
            let pr = self.selected_pr()?;
            return Some(Effect::Shell {
                command: binding.command.render(pr),
                refresh: false,
            });
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => Some(Effect::Quit),
            KeyCode::Char('j') | KeyCode::Down => self.select(|selected, _| selected + 1),
            KeyCode::Char('k') | KeyCode::Up => {
                self.select(|selected, _| selected.saturating_sub(1))
            }
            KeyCode::Char('g') | KeyCode::Home => self.select(|_, _| 0),
            KeyCode::Char('G') | KeyCode::End => self.select(|_, last| last),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => self.switch(1),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => {
                self.switch(self.sections.len() - 1)
            }
            KeyCode::Char(' ') | KeyCode::Enter => self.toggle_fold(),
            KeyCode::Char('p') => {
                self.preview = !self.preview;
                None
            }
            KeyCode::Char('d') if ctrl => self.scroll_preview(10),
            KeyCode::Char('u') if ctrl => self.scroll_preview(-10),
            KeyCode::Char('r') => Some(self.fetch_current()),
            KeyCode::Char('R') => Some(self.fetch_all()),
            _ if !ctrl => self.pr_action(key.code),
            _ => None,
        }
    }

    /// Keys that act on the selected PR.
    fn pr_action(&mut self, code: KeyCode) -> Option<Effect> {
        let pr = self.selected_pr()?.clone();
        let target = format!("-R '{}' {}", pr.repo, pr.number);
        match code {
            KeyCode::Char('o') => Some(Effect::OpenUrl(pr.url)),
            KeyCode::Char('y') => Some(Effect::Copy(pr.url)),
            KeyCode::Char('d') => Some(Effect::Shell {
                command: format!("gh pr diff --color always {target} | less -R"),
                refresh: false,
            }),
            KeyCode::Char('m') => Some(Effect::Shell {
                command: format!("gh pr merge {target}"),
                refresh: true,
            }),
            KeyCode::Char('v') => {
                self.confirm = Some(Confirm::Approve(Box::new(pr)));
                None
            }
            _ => None,
        }
    }

    fn select(&mut self, to: impl FnOnce(usize, usize) -> usize) -> Option<Effect> {
        let last = self.rows().len().saturating_sub(1);
        let view = &mut self.sections[self.tab];
        view.selected = to(view.selected, last).min(last);
        self.preview_scroll = 0;
        None
    }

    fn switch(&mut self, by: usize) -> Option<Effect> {
        self.tab = (self.tab + by) % self.sections.len();
        self.preview_scroll = 0;
        None
    }

    fn toggle_fold(&mut self) -> Option<Effect> {
        let row = self.rows().into_iter().nth(self.selected())?;
        if row.has_children {
            let id = self.current().stacks.pr(row.index).id.clone();
            if !self.folded.remove(&id) {
                self.folded.insert(id);
            }
        }
        None
    }

    fn scroll_preview(&mut self, by: i16) -> Option<Effect> {
        self.preview_scroll = self.preview_scroll.saturating_add_signed(by);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(app: &mut App, code: KeyCode) -> Option<Effect> {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    /// One section holding the chain 1 ← 2 ← 3 and a lone PR 9.
    fn app() -> App {
        let mut app = App::new(Config::default());
        app.fetched(
            0,
            Ok(vec![
                PullRequest::fixture(1, "o/r", "a", "main"),
                PullRequest::fixture(2, "o/r", "b", "a"),
                PullRequest::fixture(3, "o/r", "c", "b"),
                PullRequest::fixture(9, "o/r", "z", "main"),
            ]),
        );
        app
    }

    fn selected(app: &App) -> u64 {
        app.selected_pr().unwrap().number
    }

    #[test]
    fn folding_the_selected_stack_skips_its_rows() {
        let mut app = app();
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(selected(&app), 9);
        press(&mut app, KeyCode::Char('G'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(selected(&app), 9);
    }

    #[test]
    fn a_refresh_keeps_the_selected_pr() {
        let mut app = app();
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(selected(&app), 2);
        app.fetched(
            0,
            Ok(vec![
                PullRequest::fixture(0, "o/r", "new", "main"),
                PullRequest::fixture(2, "o/r", "b", "a"),
            ]),
        );
        assert_eq!(selected(&app), 2);
    }

    #[test]
    fn approving_waits_for_y() {
        let mut app = app();
        assert_eq!(press(&mut app, KeyCode::Char('v')), None);
        assert!(app.status().unwrap().starts_with("Approve #1"));
        assert_eq!(press(&mut app, KeyCode::Char('n')), None);
        assert_eq!(app.status(), None);
        press(&mut app, KeyCode::Char('v'));
        let Some(Effect::Approve(pr)) = press(&mut app, KeyCode::Char('y')) else {
            panic!("no approve effect");
        };
        assert_eq!(pr.number, 1);
    }

    #[test]
    fn custom_keybindings_win_over_builtins() {
        let config: Config = toml::from_str(
            "[[keybindings]]\nkey = \"o\"\nname = \"echo\"\ncommand = \"echo {number}\"",
        )
        .unwrap();
        let mut app = App::new(config);
        app.fetched(0, Ok(vec![PullRequest::fixture(4, "o/r", "a", "main")]));
        assert_eq!(
            press(&mut app, KeyCode::Char('o')),
            Some(Effect::Shell {
                command: "echo '4'".to_owned(),
                refresh: false
            })
        );
    }
}
