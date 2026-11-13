use std::time::{Duration, SystemTime};

/// An open pull request as a dashboard row shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    /// GraphQL node id, which mutations take.
    pub id: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub body: String,
    /// `owner/name`.
    pub repo: String,
    pub author: String,
    pub head: String,
    pub base: String,
    pub draft: bool,
    pub review: Review,
    pub checks: Checks,
    pub additions: u64,
    pub deletions: u64,
    pub updated_at: SystemTime,
}

#[cfg(test)]
impl PullRequest {
    /// A PR whose node id is `PR_<number>`.
    pub fn fixture(number: u64, repo: &str, head: &str, base: &str) -> Self {
        PullRequest {
            id: format!("PR_{number}"),
            number,
            title: format!("pr {number}"),
            url: format!("https://github.com/{repo}/pull/{number}"),
            body: String::new(),
            repo: repo.to_owned(),
            author: "me".to_owned(),
            head: head.to_owned(),
            base: base.to_owned(),
            draft: false,
            review: Review::None,
            checks: Checks::None,
            additions: 0,
            deletions: 0,
            updated_at: SystemTime::UNIX_EPOCH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    None,
    Required,
    Approved,
    ChangesRequested,
}

/// The rolled-up state of the head commit's checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checks {
    None,
    Pending,
    Success,
    Failure,
}

/// Parses GitHub's `DateTime`, which is always UTC: `2026-11-13T16:45:21Z`.
pub fn parse_timestamp(s: &str) -> Option<SystemTime> {
    let (date, time) = s.strip_suffix('Z')?.split_once('T')?;
    let mut date = date.splitn(3, '-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let mut time = time.splitn(3, ':');
    let (hour, minute) = (
        time.next()?.parse::<u64>().ok()?,
        time.next()?.parse::<u64>().ok()?,
    );
    // Drop fractional seconds.
    let second = time.next()?.split('.').next()?.parse::<u64>().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = u64::try_from(days_from_civil(year, month, day)).ok()?;
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(s: &str) -> u64 {
        parse_timestamp(s)
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn parses_github_timestamps() {
        assert_eq!(secs("1970-01-01T00:00:00Z"), 0);
        assert_eq!(secs("2000-03-01T00:00:00Z"), 951_868_800);
        assert_eq!(secs("2026-11-13T16:45:21Z"), 1_794_588_321);
        assert_eq!(secs("2026-11-13T16:45:21.250Z"), 1_794_588_321);
    }

    #[test]
    fn rejects_malformed_timestamps() {
        for s in [
            "",
            "2026-11-13",
            "2026-11-13T16:45:21",
            "2026-13-01T00:00:00Z",
            "1969-12-31T23:59:59Z",
        ] {
            assert_eq!(parse_timestamp(s), None, "{s}");
        }
    }
}
