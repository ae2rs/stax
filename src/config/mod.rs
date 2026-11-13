//! The user's `config.toml`: sections and custom keybindings.

mod key;
mod settings;
mod template;

pub use key::Key;
pub use settings::{Config, Keybinding, Section};
pub use template::CommandTemplate;
