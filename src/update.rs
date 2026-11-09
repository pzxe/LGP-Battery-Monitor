//! Update check against the public GitHub Releases API (user-initiated only).

use std::time::Duration;

pub const REPO: &str = "pzxe/LGP-Battery-Monitor";
pub const RELEASES_URL: &str = "https://github.com/pzxe/LGP-Battery-Monitor/releases";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available { version: String, url: String },
    Failed(String),
}

/// (major, minor, patch, prerelease-number-or-MAX-for-stable)
fn parse_version(v: &str) -> Option<(u64, u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (v, None),
    };
    let mut it = core.split('.').map(|x| x.parse::<u64>());
    let (a, b, c) = (it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?);
    let pre_n = match pre {
        None => u64::MAX,
        Some(p) => p
            .rsplit('.')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0),
    };
    Some((a, b, c, pre_n))
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

pub fn check(current: &str) -> UpdateStatus {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=5");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let body = match agent
        .get(&url)
        .header("User-Agent", "LGP-Battery-Monitor-update-check")
        .header("Accept", "application/vnd.github+json")
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
    {
        Ok(b) => b,
        Err(e) => return UpdateStatus::Failed(e.to_string()),
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(j) => j,
        Err(e) => return UpdateStatus::Failed(e.to_string()),
    };
    let best = json
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?.to_string();
            let url = r["html_url"].as_str()?.to_string();
            Some((parse_version(&tag)?, tag, url))
        })
        .max_by_key(|(v, _, _)| *v);
    match best {
        Some((_, tag, url)) if is_newer(current, &tag) => UpdateStatus::Available {
            version: tag.trim_start_matches('v').to_string(),
            url,
        },
        _ => UpdateStatus::UpToDate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering() {
        assert!(is_newer("0.1.0-beta.1", "v0.1.0-beta.2"));
        assert!(is_newer("0.1.0-beta.2", "v0.1.0"));
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "v0.1.0-rc.1"));
        assert!(!is_newer("garbage", "0.1.0"));
    }
}
