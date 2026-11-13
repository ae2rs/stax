use std::collections::HashMap;
use std::io;
use std::process::Stdio;

use anyhow::{Context, Result};
use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::task::{AbortHandle, JoinSet};

use super::state::{App, Effect};
use crate::config::Config;
use crate::github::{Client, PullRequest};
use crate::ui;

/// Work that runs beside the dashboard and reports back into the state.
enum Done {
    Fetched(usize, Result<Vec<PullRequest>>),
    Approved(u64, Result<()>),
    /// A helper like `open` or `pbcopy` finished; the string names it for errors.
    Helper(&'static str, Result<()>),
}

/// Runs the dashboard until the user quits.
pub async fn run(config: Config, client: Client) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = drive(&mut terminal, App::new(config), client).await;
    ratatui::restore();
    result
}

async fn drive(terminal: &mut DefaultTerminal, mut app: App, client: Client) -> Result<()> {
    // Owns every background task; dropping it on return aborts what is left.
    let mut tasks = JoinSet::new();
    let mut fetches: HashMap<usize, AbortHandle> = HashMap::new();
    let mut pending = vec![app.fetch_all()];
    loop {
        // A child process needs the terminal's input to itself, so the event stream (and the
        // thread it reads input on) lives only while the dashboard shows.
        let mut events = EventStream::new();
        let (command, refresh, done) = 'events: loop {
            while let Some(effect) = pending.pop() {
                match effect {
                    Effect::Quit => return Ok(()),
                    Effect::Shell {
                        command,
                        refresh,
                        done,
                    } => break 'events (command, refresh, done),
                    Effect::Fetch(indices) => {
                        for index in indices {
                            let section = app.sections()[index].section.clone();
                            let client = client.clone();
                            let handle = tasks.spawn(async move {
                                Done::Fetched(
                                    index,
                                    client.search(&section.filters, section.limit).await,
                                )
                            });
                            if let Some(stale) = fetches.insert(index, handle) {
                                stale.abort();
                            }
                        }
                    }
                    Effect::Approve(pr) => {
                        let client = client.clone();
                        tasks.spawn(
                            async move { Done::Approved(pr.number, client.approve(&pr).await) },
                        );
                    }
                    Effect::OpenUrl(url) => {
                        tasks.spawn(
                            async move { Done::Helper(OPEN, helper(OPEN, &url, None).await) },
                        );
                    }
                    Effect::Copy(text) => {
                        tasks.spawn(async move {
                            Done::Helper(COPY, helper(COPY, "", Some(&text)).await)
                        });
                    }
                }
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
            tokio::select! {
                event = events.next() => match event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        pending.extend(app.handle_key(key));
                    }
                    // Anything else, such as a resize, only needs the redraw.
                    Some(Ok(_)) => {}
                    Some(Err(err)) => return Err(err).context("cannot read the terminal"),
                    None => return Ok(()),
                },
                Some(done) = tasks.join_next(), if !tasks.is_empty() => match done {
                    Ok(Done::Fetched(index, result)) => app.fetched(index, result),
                    Ok(Done::Approved(number, result)) => match result {
                        Ok(()) => {
                            app.set_status(format!("Approved #{number}"));
                            pending.push(app.fetch_current());
                        }
                        Err(err) => app.set_status(format!("Cannot approve #{number}: {err:#}")),
                    },
                    Ok(Done::Helper(name, result)) => {
                        if let Err(err) = result {
                            app.set_status(format!("{name}: {err:#}"));
                        }
                    }
                    Err(err) if err.is_cancelled() => {}
                    Err(err) => return Err(err).context("a background task failed"),
                },
            }
        };
        drop(events);

        ratatui::restore();
        let status = Command::new("sh").arg("-c").arg(&command).status().await;
        terminal::enable_raw_mode()?;
        crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
        terminal.clear()?;
        match status {
            Ok(status) if status.success() => {
                if let Some(done) = done {
                    app.set_status(done);
                }
            }
            Ok(status) => app.set_status(format!("`{command}` failed: {status}")),
            Err(err) => app.set_status(format!("cannot run `{command}`: {err}")),
        }
        if refresh {
            pending.push(app.fetch_current());
        }
    }
}

const OPEN: &str = if cfg!(target_os = "macos") {
    "open"
} else {
    "xdg-open"
};
const COPY: &str = if cfg!(target_os = "macos") {
    "pbcopy"
} else {
    "wl-copy"
};

/// Runs a desktop helper without the terminal, optionally feeding it `input`.
async fn helper(program: &str, arg: &str, input: Option<&str>) -> Result<()> {
    let mut command = Command::new(program);
    if !arg.is_empty() {
        command.arg(arg);
    }
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input.as_bytes()).await?;
    }
    let status = child.wait().await?;
    anyhow::ensure!(status.success(), "exited with {status}");
    Ok(())
}
