pub mod state;
pub mod version;

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use thiserror::Error;

use crate::config;
pub use state::Available;
use state::State;
pub use version::Version;

const RELEASES_URL: &str = "https://api.github.com/repos/remcostoeten/kiekje/releases?per_page=20";
const USER_AGENT: &str = concat!("kiekje/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REPLY_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    UpToDate,
    Newer(Available),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Automatic {
    Off,
    NotDue,
    Failed,
    UpToDate,
    Newer { available: Available, notify: bool },
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("the release feed could not be fetched: {0}")]
    Request(#[from] ureq::Error),
    #[error("the release feed is not valid: {0}")]
    Parse(#[from] serde_json::Error),
    #[error(transparent)]
    Version(#[from] version::VersionError),
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
}

/// The version of this build.
///
/// # Examples
///
/// ```ignore
/// let current = update::current()?;
/// ```
pub fn current() -> Result<Version, version::VersionError> {
    Version::parse(env!("CARGO_PKG_VERSION"))
}

/// Asks api.github.com for the releases and compares the newest with this build.
///
/// Blocks for up to ten seconds, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let outcome = gio::spawn_blocking(update::fetch).await;
/// ```
pub fn fetch() -> Result<Outcome, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .build()
        .into();
    let text = agent
        .get(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .call()?
        .body_mut()
        .with_config()
        .limit(MAX_REPLY_BYTES)
        .read_to_string()?;
    outcome(&text, &current()?)
}

fn outcome(feed: &str, current: &Version) -> Result<Outcome, UpdateError> {
    let releases: Vec<Release> = serde_json::from_str(feed)?;
    Ok(newest(&releases, current).map_or(Outcome::UpToDate, Outcome::Newer))
}

fn newest(releases: &[Release], current: &Version) -> Option<Available> {
    releases
        .iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            Version::parse(&release.tag_name)
                .ok()
                .map(|version| (version, release))
        })
        .filter(|(version, _)| current.is_prerelease() || !version.is_prerelease())
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .filter(|(version, _)| version > current)
        .map(|(version, release)| Available {
            version: version.to_string(),
            url: release.html_url.clone(),
        })
}

/// Checks right away, whatever the setting says, and records the result.
///
/// Blocks on the network and the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let outcome = gio::spawn_blocking(|| update::check_now(&update::state::path())).await;
/// ```
pub fn check_now(state_path: &Path) -> Result<Outcome, UpdateError> {
    let found = fetch()?;
    let now = unix_now();
    let recorded = state::update(state_path, |state| {
        state.last_checked = Some(now);
        record(state, &found);
        if let Outcome::Newer(available) = &found {
            state.notified = Some(available.version.clone());
        }
    });
    if let Err(error) = recorded {
        eprintln!("kiekje: the update check result was not stored: {error}");
    }
    Ok(found)
}

/// Runs the opt-in daily check when `[general] check_for_updates` is on and a day has passed.
///
/// A newer version asks for a notification only once. Failures are logged and count as a check,
/// so an offline machine does not retry every hour. Blocks on the network and the disk, so call
/// it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let result = gio::spawn_blocking(|| update::automatic(&config::path(), &update::state::path())).await;
/// ```
pub fn automatic(config_path: &Path, state_path: &Path) -> Automatic {
    let enabled = config::load_from(config_path)
        .map(|config| config.unwrap_or_default().general.check_for_updates)
        .unwrap_or(false);
    if !enabled {
        return Automatic::Off;
    }
    let last_checked = state::load_from(state_path)
        .map(|state| state.last_checked)
        .unwrap_or(None);
    let now = unix_now();
    if !state::is_due(last_checked, now) {
        return Automatic::NotDue;
    }
    let found = fetch();
    let mut result = Automatic::Failed;
    let recorded = state::update(state_path, |state| {
        state.last_checked = Some(now);
        result = match &found {
            Ok(found) => {
                record(state, found);
                decide(state, found)
            }
            Err(_) => Automatic::Failed,
        };
    });
    if let Err(error) = &found {
        eprintln!("kiekje: could not check for updates: {error}");
    }
    if let Err(error) = recorded {
        eprintln!("kiekje: the update check result was not stored: {error}");
    }
    result
}

fn record(state: &mut State, found: &Outcome) {
    state.available = match found {
        Outcome::UpToDate => None,
        Outcome::Newer(available) => Some(available.clone()),
    };
}

fn decide(state: &mut State, found: &Outcome) -> Automatic {
    match found {
        Outcome::UpToDate => Automatic::UpToDate,
        Outcome::Newer(available) => {
            let notify = state.notified.as_deref() != Some(available.version.as_str());
            state.notified = Some(available.version.clone());
            Automatic::Newer {
                available: available.clone(),
                notify,
            }
        }
    }
}

/// Seconds since the Unix epoch, or 0 when the clock is before it.
///
/// # Examples
///
/// ```ignore
/// state.last_checked = Some(unix_now());
/// ```
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"[
        {"tag_name": "v0.1.0-alpha.10", "html_url": "https://example.test/a10", "draft": false, "prerelease": true},
        {"tag_name": "v0.1.0-alpha.9", "html_url": "https://example.test/a9", "draft": false, "prerelease": true},
        {"tag_name": "v0.1.0-alpha.11", "html_url": "https://example.test/a11", "draft": true, "prerelease": true},
        {"tag_name": "nightly", "html_url": "https://example.test/nightly", "draft": false, "prerelease": true}
    ]"#;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn finds_the_newest_published_prerelease() {
        let found = outcome(FEED, &version("0.1.0-alpha.5")).unwrap();
        assert_eq!(
            found,
            Outcome::Newer(Available {
                version: "0.1.0-alpha.10".into(),
                url: "https://example.test/a10".into(),
            })
        );
    }

    #[test]
    fn reports_up_to_date_on_the_newest_version() {
        let found = outcome(FEED, &version("0.1.0-alpha.10")).unwrap();
        assert_eq!(found, Outcome::UpToDate);
    }

    #[test]
    fn offers_a_stable_build_only_stable_releases() {
        let feed = r#"[
            {"tag_name": "v1.1.0-rc.1", "html_url": "https://example.test/rc"},
            {"tag_name": "v1.0.1", "html_url": "https://example.test/101"}
        ]"#;
        let found = outcome(feed, &version("1.0.0")).unwrap();
        assert!(matches!(found, Outcome::Newer(available) if available.version == "1.0.1"));
        assert_eq!(outcome(feed, &version("1.0.1")).unwrap(), Outcome::UpToDate);
    }

    #[test]
    fn rejects_a_feed_that_is_not_a_release_list() {
        let parsed = outcome(
            r#"{"message": "API rate limit exceeded"}"#,
            &version("1.0.0"),
        );
        assert!(matches!(parsed, Err(UpdateError::Parse(_))));
    }

    #[test]
    fn notifies_once_per_newer_version() {
        let mut state = State::default();
        let found = Outcome::Newer(Available {
            version: "0.1.0-alpha.6".into(),
            url: "https://example.test/a6".into(),
        });
        assert!(matches!(
            decide(&mut state, &found),
            Automatic::Newer { notify: true, .. }
        ));
        assert!(matches!(
            decide(&mut state, &found),
            Automatic::Newer { notify: false, .. }
        ));
        let later = Outcome::Newer(Available {
            version: "0.1.0-alpha.7".into(),
            url: "https://example.test/a7".into(),
        });
        assert!(matches!(
            decide(&mut state, &later),
            Automatic::Newer { notify: true, .. }
        ));
    }

    #[test]
    fn stays_off_unless_the_setting_is_on() {
        let directory =
            std::env::temp_dir().join(format!("kiekje-update-off-{}", std::process::id()));
        let config_path = directory.join("config.toml");
        let state_path = directory.join("update-check.toml");
        assert_eq!(automatic(&config_path, &state_path), Automatic::Off);
        config::save_to(&config_path, &config::Config::default()).unwrap();
        assert_eq!(automatic(&config_path, &state_path), Automatic::Off);
        assert!(!state_path.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn waits_when_the_last_check_is_recent() {
        let directory =
            std::env::temp_dir().join(format!("kiekje-update-recent-{}", std::process::id()));
        let config_path = directory.join("config.toml");
        let state_path = directory.join("update-check.toml");
        let mut config = config::Config::default();
        config.general.check_for_updates = true;
        config::save_to(&config_path, &config).unwrap();
        state::update(&state_path, |state| state.last_checked = Some(unix_now())).unwrap();
        assert_eq!(automatic(&config_path, &state_path), Automatic::NotDue);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
