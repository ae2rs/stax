mod app;
mod config;
mod github;
mod stack;
mod ui;

use std::collections::HashSet;
use std::env;

use anyhow::{Result, bail};

use crate::config::Config;
use crate::github::Client;
use crate::stack::Stacks;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let client = Client::from_gh().await?;
    match env::args().nth(1).as_deref() {
        None => app::run(config, client).await,
        Some("list") => list(&config, &client).await,
        Some(_) => bail!("usage: stax [list]"),
    }
}

/// Prints every section's stacks as text, for scripts and for checking a config.
async fn list(config: &Config, client: &Client) -> Result<()> {
    for section in &config.sections {
        let stacks = Stacks::new(client.search(&section.filters, section.limit).await?);
        println!("{} ({})", section.title, stacks.len());
        for row in stacks.rows(&HashSet::new()) {
            let pr = stacks.pr(row.index);
            println!("  {}#{} {} ({})", row.guide, pr.number, pr.title, pr.repo);
        }
    }
    Ok(())
}
