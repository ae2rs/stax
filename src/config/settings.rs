use std::path::PathBuf;
use std::{env, fs, io};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::{CommandTemplate, Key, RepoPaths};

/// The most results one GitHub search request returns.
const MAX_LIMIT: u32 = 100;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_sections")]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub keybindings: Vec<Keybinding>,
    #[serde(default)]
    pub repo_paths: RepoPaths,
}

/// A dashboard tab: the PRs one GitHub search returns.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub title: String,
    /// GitHub search qualifiers; stax adds `is:pr archived:false`.
    pub filters: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

/// A key that runs a shell command for the selected PR, with the dashboard suspended.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keybinding {
    pub key: Key,
    pub name: String,
    pub command: CommandTemplate,
}

impl Config {
    /// Reads `$XDG_CONFIG_HOME/stax/config.toml`, falling back to `~/.config`; a missing file
    /// gives the defaults.
    pub fn load() -> Result<Self> {
        let path = path()?;
        match fs::read_to_string(&path) {
            Ok(text) => {
                Self::parse(&text).with_context(|| format!("invalid config {}", path.display()))
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    fn parse(text: &str) -> Result<Self> {
        let config: Config = toml::from_str(text)?;
        if config.sections.is_empty() {
            bail!("`sections` is empty");
        }
        for section in &config.sections {
            if !(1..=MAX_LIMIT).contains(&section.limit) {
                bail!(
                    "section `{}`: limit must be 1 to {MAX_LIMIT}",
                    section.title
                );
            }
        }
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sections: default_sections(),
            keybindings: Vec::new(),
            repo_paths: RepoPaths::default(),
        }
    }
}

fn path() -> Result<PathBuf> {
    let base = match env::var_os("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => env::home_dir()
            .context("no home directory")?
            .join(".config"),
    };
    Ok(base.join("stax").join("config.toml"))
}

fn default_sections() -> Vec<Section> {
    let section = |title: &str, filters: &str| Section {
        title: title.to_owned(),
        filters: filters.to_owned(),
        limit: default_limit(),
    };
    vec![
        section("My PRs", "is:open author:@me"),
        section("Review requested", "is:open review-requested:@me"),
        section("Involved", "is:open involves:@me -author:@me"),
    ]
}

fn default_limit() -> u32 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_default_sections() {
        let config = Config::parse("").unwrap();
        assert_eq!(config.sections.len(), 3);
        assert!(config.keybindings.is_empty());
    }

    #[test]
    fn example_config_parses() {
        let config = Config::parse(include_str!("../../config.example.toml")).unwrap();
        assert!(!config.sections.is_empty());
        assert!(!config.keybindings.is_empty());
    }

    #[test]
    fn rejects_bad_configs() {
        for bad in [
            "sections = []",
            "[[sections]]\ntitle = \"a\"\nfilters = \"b\"\nlimit = 0",
            "[[sections]]\ntitle = \"a\"\nfilters = \"b\"\nlimit = 101",
            "[[sections]]\ntitle = \"a\"",
            "[[keybindings]]\nkey = \"nope\"\nname = \"n\"\ncommand = \"c\"",
            "[[keybindings]]\nkey = \"x\"\nname = \"n\"\ncommand = \"echo {nope}\"",
            "colour = \"red\"",
        ] {
            assert!(Config::parse(bad).is_err(), "{bad}");
        }
    }
}
