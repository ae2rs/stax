use std::collections::HashMap;
use std::env;

use serde::Deserialize;

/// Where repos are checked out: `"owner/name" = "~/src/name"`, or `"owner/*" = "~/src/*"` for
/// every repo of an owner, where `*` stands for the repo's name.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct RepoPaths(HashMap<String, String>);

impl RepoPaths {
    /// The local checkout of `repo` (`owner/name`), with `~` expanded.
    pub fn get(&self, repo: &str) -> Option<String> {
        let (owner, name) = repo.split_once('/')?;
        let path = match self.0.get(repo) {
            Some(path) => path.clone(),
            None => self.0.get(&format!("{owner}/*"))?.replace('*', name),
        };
        Some(match path.strip_prefix("~/") {
            Some(rest) => format!("{}/{rest}", env::home_dir()?.display()),
            None => path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_names_win_over_owner_wildcards() {
        let paths = RepoPaths(HashMap::from([
            ("o/r".to_owned(), "/work/r".to_owned()),
            ("o/*".to_owned(), "/src/*".to_owned()),
        ]));
        assert_eq!(paths.get("o/r").as_deref(), Some("/work/r"));
        assert_eq!(paths.get("o/x").as_deref(), Some("/src/x"));
        assert_eq!(paths.get("p/x"), None);
    }

    #[test]
    fn expands_home() {
        let paths = RepoPaths(HashMap::from([("o/r".to_owned(), "~/r".to_owned())]));
        let home = env::home_dir().unwrap();
        assert_eq!(paths.get("o/r"), Some(format!("{}/r", home.display())));
    }
}
