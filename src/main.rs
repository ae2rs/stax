mod config;
mod github;

use std::env;

use anyhow::{Result, bail};

use crate::config::Config;
use crate::github::Client;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let client = Client::from_gh().await?;
    match env::args().nth(1).as_deref() {
        Some("list") => {
            for section in &config.sections {
                let prs = client.search(&section.filters, section.limit).await?;
                println!("{} ({})", section.title, prs.len());
                for pr in prs {
                    println!(
                        "  {} #{} {} [{} <- {}]",
                        pr.repo, pr.number, pr.title, pr.base, pr.head
                    );
                }
            }
            Ok(())
        }
        _ => bail!("usage: stax list"),
    }
}
