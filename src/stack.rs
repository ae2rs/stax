//! Stacked PRs: a PR whose base branch is another PR's head branch, in the same repo, nests
//! under it.

use std::collections::{HashMap, HashSet};

use crate::github::PullRequest;

/// One section's PRs, arranged as a forest of stacks.
pub struct Stacks {
    prs: Vec<PullRequest>,
    /// Per PR, its children in number order.
    children: Vec<Vec<usize>>,
    /// In the order their first PR came from GitHub, so the section's sort still holds.
    roots: Vec<usize>,
}

/// A visible line of the table.
#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    /// Index into the PRs, for [`Stacks::pr`].
    pub index: usize,
    /// Tree drawing before the PR: empty for a root, `│  └─ ` and the like below it.
    pub guide: String,
    pub has_children: bool,
    /// How many PRs a fold hides under this one.
    pub hidden: usize,
}

impl Stacks {
    pub fn new(prs: Vec<PullRequest>) -> Self {
        let mut by_head = HashMap::new();
        for (i, pr) in prs.iter().enumerate() {
            by_head
                .entry((pr.repo.as_str(), pr.head.as_str()))
                .or_insert(i);
        }
        let mut parent: Vec<Option<usize>> = prs
            .iter()
            .enumerate()
            .map(|(i, pr)| {
                by_head
                    .get(&(pr.repo.as_str(), pr.base.as_str()))
                    .copied()
                    .filter(|&p| p != i)
            })
            .collect();
        // Branches can point at each other in a loop; cut it so every PR still has a root.
        for i in 0..prs.len() {
            let mut path = vec![i];
            let mut j = i;
            while let Some(p) = parent[j] {
                if path.contains(&p) {
                    parent[j] = None;
                    break;
                }
                path.push(p);
                j = p;
            }
        }

        let mut children = vec![Vec::new(); prs.len()];
        for (i, p) in parent.iter().enumerate() {
            if let Some(p) = *p {
                children[p].push(i);
            }
        }
        for kids in &mut children {
            kids.sort_by_key(|&i| prs[i].number);
        }
        let mut roots = Vec::new();
        for i in 0..prs.len() {
            let mut root = i;
            while let Some(p) = parent[root] {
                root = p;
            }
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
        Stacks {
            prs,
            children,
            roots,
        }
    }

    pub fn pr(&self, index: usize) -> &PullRequest {
        &self.prs[index]
    }

    pub fn len(&self) -> usize {
        self.prs.len()
    }

    /// The visible rows, depth first, skipping what lies under a folded PR (by node id).
    pub fn rows(&self, folded: &HashSet<String>) -> Vec<Row> {
        let mut rows = Vec::new();
        for &root in &self.roots {
            self.push_rows(root, String::new(), "", folded, &mut rows);
        }
        rows
    }

    fn push_rows(
        &self,
        index: usize,
        guide: String,
        indent: &str,
        folded: &HashSet<String>,
        rows: &mut Vec<Row>,
    ) {
        let kids = &self.children[index];
        let is_folded = !kids.is_empty() && folded.contains(&self.prs[index].id);
        rows.push(Row {
            index,
            guide,
            has_children: !kids.is_empty(),
            hidden: if is_folded {
                self.descendants(index)
            } else {
                0
            },
        });
        if is_folded {
            return;
        }
        for (k, &kid) in kids.iter().enumerate() {
            let (branch, rest) = if k + 1 == kids.len() {
                ("└─ ", "   ")
            } else {
                ("├─ ", "│  ")
            };
            self.push_rows(
                kid,
                format!("{indent}{branch}"),
                &format!("{indent}{rest}"),
                folded,
                rows,
            );
        }
    }

    fn descendants(&self, index: usize) -> usize {
        self.children[index]
            .iter()
            .map(|&kid| 1 + self.descendants(kid))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr(number: u64, repo: &str, head: &str, base: &str) -> PullRequest {
        PullRequest::fixture(number, repo, head, base)
    }

    /// Each row as `guide#number`, with `+n` when a fold hides n PRs.
    fn draw(stacks: &Stacks, folded: &[u64]) -> Vec<String> {
        let folded = folded.iter().map(|n| format!("PR_{n}")).collect();
        stacks
            .rows(&folded)
            .into_iter()
            .map(|row| {
                let hidden = if row.hidden > 0 {
                    format!(" +{}", row.hidden)
                } else {
                    String::new()
                };
                format!("{}#{}{hidden}", row.guide, stacks.pr(row.index).number)
            })
            .collect()
    }

    #[test]
    fn chains_nest_in_stack_order() {
        // GitHub returns newest first; the stack still reads bottom up.
        let stacks = Stacks::new(vec![
            pr(3, "o/r", "c", "b"),
            pr(2, "o/r", "b", "a"),
            pr(1, "o/r", "a", "main"),
        ]);
        assert_eq!(draw(&stacks, &[]), ["#1", "└─ #2", "   └─ #3"]);
    }

    #[test]
    fn siblings_branch_and_keep_drawing_the_trunk() {
        let stacks = Stacks::new(vec![
            pr(1, "o/r", "a", "main"),
            pr(2, "o/r", "b", "a"),
            pr(4, "o/r", "d", "b"),
            pr(3, "o/r", "c", "a"),
        ]);
        assert_eq!(draw(&stacks, &[]), ["#1", "├─ #2", "│  └─ #4", "└─ #3"]);
    }

    #[test]
    fn orphans_and_other_repos_stay_roots_in_github_order() {
        let stacks = Stacks::new(vec![
            pr(9, "o/r", "x", "missing-parent"),
            pr(2, "o/other", "b", "a"),
            pr(1, "o/r", "a", "main"),
        ]);
        assert_eq!(draw(&stacks, &[]), ["#9", "#2", "#1"]);
    }

    #[test]
    fn a_child_listed_first_puts_its_stack_first() {
        let stacks = Stacks::new(vec![
            pr(2, "o/r", "b", "a"),
            pr(9, "o/r", "z", "main"),
            pr(1, "o/r", "a", "main"),
        ]);
        assert_eq!(draw(&stacks, &[]), ["#1", "└─ #2", "#9"]);
    }

    #[test]
    fn folding_hides_descendants_and_counts_them() {
        let stacks = Stacks::new(vec![
            pr(1, "o/r", "a", "main"),
            pr(2, "o/r", "b", "a"),
            pr(3, "o/r", "c", "b"),
        ]);
        assert_eq!(draw(&stacks, &[1]), ["#1 +2"]);
        assert_eq!(draw(&stacks, &[2]), ["#1", "└─ #2 +1"]);
        // A fold on a leaf does nothing.
        assert_eq!(draw(&stacks, &[3]), ["#1", "└─ #2", "   └─ #3"]);
    }

    #[test]
    fn loops_are_cut_so_every_pr_shows() {
        let stacks = Stacks::new(vec![pr(1, "o/r", "a", "b"), pr(2, "o/r", "b", "a")]);
        let rows = draw(&stacks, &[]);
        assert_eq!(rows.len(), 2);
    }
}
