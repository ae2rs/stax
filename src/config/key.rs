use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

/// A key a binding fires on: `T`, `ctrl+t`, `enter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Key {
    code: KeyCode,
    ctrl: bool,
}

impl Key {
    pub fn matches(&self, event: &KeyEvent) -> bool {
        // Shift is already in an uppercase char, and terminals disagree on whether they report it.
        event.code == self.code && event.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
    }
}

impl TryFrom<String> for Key {
    type Error = String;

    fn try_from(s: String) -> Result<Self, String> {
        let (ctrl, name) = match s.strip_prefix("ctrl+") {
            Some(name) => (true, name),
            None => (false, s.as_str()),
        };
        let mut chars = name.chars();
        let code = match (chars.next(), chars.next()) {
            (Some(c), None) => KeyCode::Char(c),
            _ => match name {
                "enter" => KeyCode::Enter,
                "tab" => KeyCode::Tab,
                "esc" => KeyCode::Esc,
                "space" => KeyCode::Char(' '),
                "backspace" => KeyCode::Backspace,
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                "left" => KeyCode::Left,
                "right" => KeyCode::Right,
                _ => return Err(format!("unknown key `{s}`")),
            },
        };
        Ok(Key { code, ctrl })
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("ctrl+")?;
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Enter => f.write_str("enter"),
            KeyCode::Tab => f.write_str("tab"),
            KeyCode::Esc => f.write_str("esc"),
            KeyCode::Backspace => f.write_str("backspace"),
            KeyCode::Up => f.write_str("up"),
            KeyCode::Down => f.write_str("down"),
            KeyCode::Left => f.write_str("left"),
            KeyCode::Right => f.write_str("right"),
            code => write!(f, "{code:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Key {
        Key::try_from(s.to_owned()).unwrap()
    }

    #[test]
    fn round_trips_through_display() {
        for s in ["T", "ctrl+t", "enter", "space", "ctrl+space"] {
            assert_eq!(key(s).to_string(), s);
        }
    }

    #[test]
    fn rejects_unknown_names() {
        assert!(Key::try_from("hyper+x".to_owned()).is_err());
        assert!(Key::try_from(String::new()).is_err());
    }

    #[test]
    fn matches_uppercase_with_or_without_shift() {
        let t = key("T");
        assert!(t.matches(&KeyEvent::new(KeyCode::Char('T'), KeyModifiers::SHIFT)));
        assert!(t.matches(&KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE)));
        assert!(!t.matches(&KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)));
        assert!(!t.matches(&KeyEvent::new(KeyCode::Char('T'), KeyModifiers::CONTROL)));
    }
}
