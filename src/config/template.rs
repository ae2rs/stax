use serde::Deserialize;

use crate::github::PullRequest;

/// A shell command with `{field}` placeholders for the selected PR, checked when the config loads.
///
/// Each value goes in single-quoted, so a branch name cannot inject shell syntax. `{{` and `}}`
/// write literal braces.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandTemplate(Vec<Part>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Field(Field),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Repo,
    Number,
    Url,
    Head,
    Base,
    Author,
}

const FIELDS: [(&str, Field); 6] = [
    ("repo", Field::Repo),
    ("number", Field::Number),
    ("url", Field::Url),
    ("head", Field::Head),
    ("base", Field::Base),
    ("author", Field::Author),
];

impl CommandTemplate {
    pub fn render(&self, pr: &PullRequest) -> String {
        let mut out = String::new();
        for part in &self.0 {
            match part {
                Part::Literal(text) => out.push_str(text),
                Part::Field(field) => {
                    let number = pr.number.to_string();
                    let value = match field {
                        Field::Repo => &pr.repo,
                        Field::Number => &number,
                        Field::Url => &pr.url,
                        Field::Head => &pr.head,
                        Field::Base => &pr.base,
                        Field::Author => &pr.author,
                    };
                    out.push_str(&shell_quote(value));
                }
            }
        }
        out
    }
}

/// `s` as one single-quoted shell word.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

impl TryFrom<String> for CommandTemplate {
    type Error = String;

    fn try_from(s: String) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut rest = s.as_str();
        while let Some(i) = rest.find(['{', '}']) {
            literal.push_str(&rest[..i]);
            let tail = &rest[i..];
            if let Some(after) = tail.strip_prefix("{{") {
                literal.push('{');
                rest = after;
            } else if let Some(after) = tail.strip_prefix("}}") {
                literal.push('}');
                rest = after;
            } else if let Some(after) = tail.strip_prefix('{') {
                let (name, after) = after
                    .split_once('}')
                    .ok_or_else(|| format!("unclosed `{{` in `{s}`"))?;
                let (_, field) = FIELDS.iter().find(|(n, _)| *n == name).ok_or_else(|| {
                    let names: Vec<_> = FIELDS.iter().map(|(n, _)| *n).collect();
                    format!("unknown placeholder `{{{name}}}` in `{s}`; known: {}; write `{{{{` for a literal brace", names.join(", "))
                })?;
                parts.push(Part::Literal(std::mem::take(&mut literal)));
                parts.push(Part::Field(*field));
                rest = after;
            } else {
                return Err(format!(
                    "unmatched `}}` in `{s}`; write `}}}}` for a literal brace"
                ));
            }
        }
        literal.push_str(rest);
        parts.push(Part::Literal(literal));
        parts.retain(|part| !matches!(part, Part::Literal(text) if text.is_empty()));
        Ok(CommandTemplate(parts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(template: &str) -> String {
        CommandTemplate::try_from(template.to_owned())
            .unwrap()
            .render(&PullRequest::fixture(7, "o/r", "it's;rm -rf", "main"))
    }

    #[test]
    fn substitutes_quoted_fields() {
        assert_eq!(
            render("gh pr view -R {repo} {number}"),
            "gh pr view -R 'o/r' '7'"
        );
        assert_eq!(render("echo {head}"), r"echo 'it'\''s;rm -rf'");
    }

    #[test]
    fn doubled_braces_are_literal() {
        assert_eq!(render("echo {{a,b}} ${{HOME}}"), "echo {a,b} ${HOME}");
    }

    #[test]
    fn rejects_bad_placeholders() {
        for bad in ["echo {nope}", "echo {repo", "echo }", "echo ${HOME}"] {
            assert!(CommandTemplate::try_from(bad.to_owned()).is_err(), "{bad}");
        }
    }
}
