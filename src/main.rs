mod config;
mod github;
mod stack;

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
        Some("list") => {
            for section in &config.sections {
                let prs = client.search(&section.filters, section.limit).await?;
                println!("{} ({})", section.title, prs.len());
                let stacks = Stacks::new(prs);
                for row in stacks.rows(&HashSet::new()) {
                    let pr = stacks.pr(row.index);
                    println!("  {}#{} {} ({})", row.guide, pr.number, pr.title, pr.repo);
                }
            }
            Ok(())
        }
        _ => bail!("usage: stax list"),
    }
}
