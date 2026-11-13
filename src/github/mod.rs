//! GitHub's GraphQL API and the pull requests it returns.

mod client;
mod pull_request;

pub use client::Client;
pub use pull_request::{Checks, PullRequest, Review};
