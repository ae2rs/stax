//! The user's `config.toml`: sections, custom keybindings and where repos are checked out.

mod key;
mod repo_paths;
mod settings;
mod template;

pub use key::Key;
pub use repo_paths::RepoPaths;
pub use settings::{Config, Keybinding, Section};
pub use template::{CommandTemplate, shell_quote};
