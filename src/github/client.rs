use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::process::Command;

use super::pull_request::{Checks, PullRequest, Review, parse_timestamp};

const ENDPOINT: &str = "https://api.github.com/graphql";

const SEARCH: &str = r#"
query($query: String!, $first: Int!) {
  search(query: $query, type: ISSUE, first: $first) {
    nodes {
      ... on PullRequest {
        id number title url body isDraft headRefName baseRefName additions deletions updatedAt
        reviewDecision
        author { login }
        repository { nameWithOwner }
        commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
      }
    }
  }
}"#;

const APPROVE: &str = r#"
mutation($id: ID!) {
  addPullRequestReview(input: { pullRequestId: $id, event: APPROVE }) { clientMutationId }
}"#;

/// GitHub's GraphQL API, authenticated as the `gh` CLI's user.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    token: String,
}

impl Client {
    pub async fn from_gh() -> Result<Self> {
        let output = Command::new("gh")
            .args(["auth", "token"])
            .output()
            .await
            .context("cannot run `gh auth token`; is the gh CLI installed?")?;
        if !output.status.success() {
            bail!(
                "`gh auth token` failed; run `gh auth login`: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let token =
            String::from_utf8(output.stdout).context("`gh auth token` printed non-UTF-8")?;
        let http = reqwest::Client::builder()
            .user_agent(concat!("stax/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Client {
            http,
            token: token.trim().to_owned(),
        })
    }

    /// The PRs a search returns, in GitHub's order (newest first).
    pub async fn search(&self, filters: &str, limit: u32) -> Result<Vec<PullRequest>> {
        let query = format!("is:pr archived:false {filters}");
        let data: SearchData = self
            .request(SEARCH, json!({ "query": query, "first": limit }))
            .await?;
        Ok(data
            .search
            .nodes
            .into_iter()
            .map(PullRequest::from)
            .collect())
    }

    pub async fn approve(&self, pr: &PullRequest) -> Result<()> {
        let _: Value = self.request(APPROVE, json!({ "id": pr.id })).await?;
        Ok(())
    }

    async fn request<T: DeserializeOwned>(&self, query: &str, variables: Value) -> Result<T> {
        let response = self
            .http
            .post(ENDPOINT)
            .bearer_auth(&self.token)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .context("cannot reach GitHub")?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "GitHub answered {status}: {}",
                response.text().await.unwrap_or_default()
            );
        }
        let body: Response<T> = response
            .json()
            .await
            .context("unexpected GitHub response")?;
        if let Some(errors) = body.errors.filter(|errors| !errors.is_empty()) {
            let messages: Vec<_> = errors.into_iter().map(|error| error.message).collect();
            bail!("GitHub: {}", messages.join("; "));
        }
        body.data.ok_or_else(|| anyhow!("GitHub returned no data"))
    }
}

#[derive(Deserialize)]
struct Response<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Deserialize)]
struct SearchData {
    search: Search,
}

#[derive(Deserialize)]
struct Search {
    /// `is:pr` keeps out issues, which would come back as empty objects.
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    id: String,
    number: u64,
    title: String,
    url: String,
    body: String,
    is_draft: bool,
    head_ref_name: String,
    base_ref_name: String,
    additions: u64,
    deletions: u64,
    updated_at: String,
    review_decision: Option<String>,
    /// Absent when the account was deleted.
    author: Option<Login>,
    repository: Repository,
    commits: Commits,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Repository {
    name_with_owner: String,
}

#[derive(Deserialize)]
struct Commits {
    nodes: Vec<CommitNode>,
}

#[derive(Deserialize)]
struct CommitNode {
    commit: Commit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Commit {
    status_check_rollup: Option<Rollup>,
}

#[derive(Deserialize)]
struct Rollup {
    state: String,
}

impl From<Node> for PullRequest {
    fn from(node: Node) -> Self {
        let review = match node.review_decision.as_deref() {
            Some("APPROVED") => Review::Approved,
            Some("CHANGES_REQUESTED") => Review::ChangesRequested,
            Some("REVIEW_REQUIRED") => Review::Required,
            _ => Review::None,
        };
        let rollup = node
            .commits
            .nodes
            .into_iter()
            .next()
            .and_then(|node| node.commit.status_check_rollup);
        let checks = match rollup.as_ref().map(|rollup| rollup.state.as_str()) {
            Some("SUCCESS") => Checks::Success,
            Some("FAILURE" | "ERROR") => Checks::Failure,
            Some("PENDING" | "EXPECTED") => Checks::Pending,
            _ => Checks::None,
        };
        PullRequest {
            id: node.id,
            number: node.number,
            title: node.title,
            url: node.url,
            body: node.body,
            repo: node.repository.name_with_owner,
            author: node
                .author
                .map_or_else(|| "ghost".to_owned(), |author| author.login),
            head: node.head_ref_name,
            base: node.base_ref_name,
            draft: node.is_draft,
            review,
            checks,
            additions: node.additions,
            deletions: node.deletions,
            updated_at: parse_timestamp(&node.updated_at)
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        }
    }
}
