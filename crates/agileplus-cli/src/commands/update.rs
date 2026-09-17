// SPDX-License-Identifier: MIT OR Apache-2.0
//! `agileplus update` — self-update mechanism.
//!
//! Checks for new releases on GitHub and performs binary updates.
//! Uses the GitHub Releases API for `KooshaPari/AgilePlus`.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Deserialize;

// ── GitHub API types ─────────────────────────────────────────────────────────

const GH_REPO: &str = "KooshaPari/AgilePlus";
const GH_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    prerelease: bool,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    #[serde(rename = "browser_download_url")]
    download_url: String,
    size: u64,
}

// ── Args ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Only check for updates; do not download or install.
    #[arg(short = 'c', long)]
    pub check_only: bool,

    /// Force update even if the same version is already installed.
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Include pre-release versions when checking for updates.
    #[arg(short = 'p', long)]
    pub prerelease: bool,
}

// ── Entry point ──────────────────────────────────────────────────────────────

pub async fn run(args: &UpdateArgs) -> Result<()> {
    let current_ver = env!("CARGO_PKG_VERSION");
    println!("Current version: v{current_ver}");

    // 1. Build the HTTP client with a reasonable User-Agent (GitHub API requires one).
    let client = reqwest::Client::builder()
        .user_agent("agileplus-cli/1.0")
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client")?;

    // 2. Fetch release data from GitHub.
    let releases = fetch_releases(&client, args.prerelease).await?;

    // 3. Find the latest applicable release.
    let latest = match find_latest(&releases) {
        Some(r) => r,
        None => {
            println!("No releases found.");
            return Ok(());
        }
    };

    let latest_tag = latest.tag_name.trim_start_matches('v');
    println!("Latest release: v{latest_tag}");

    // 4. Compare versions.
    let cmp = compare_versions(current_ver, latest_tag);

    if args.force {
        println!("--force set; proceeding with update regardless of version comparison.");
    } else if cmp == std::cmp::Ordering::Greater || cmp == std::cmp::Ordering::Equal {
        println!("Already up to date (v{current_ver}).");
        println!("  Use --force to reinstall or --prerelease to include pre-releases.");
        return Ok(());
    }

    // 5. If --check-only, stop here.
    if args.check_only {
        println!("Update available: v{latest_tag} (current: v{current_ver})");
        println!("  Run `agileplus update` without --check-only to install.");
        return Ok(());
    }

    // 6. Find the right binary asset for the current platform.
    let target = target_binary_name();
    let asset = match latest.assets.iter().find(|a| a.name == target) {
        Some(a) => a,
        None => {
            bail!(
                "no release asset found for this platform ({target}); \
                 available assets: {}",
                latest
                    .assets
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    };

    // 7. Download to a temp file.
    let current_exe = std::env::current_exe().context("failed to get current executable path")?;
    let tmp_path = download_binary(
        &client,
        &asset.download_url,
        current_exe.parent().unwrap_or(&PathBuf::from(".")),
    )
    .await?;

    // 8. Replace the current binary.
    replace_binary(&tmp_path, &current_exe)?;

    println!("Update complete: v{current_ver} → v{latest_tag}");
    println!("  Restart the CLI to use the new version.");

    Ok(())
}

// ── Fetch releases ───────────────────────────────────────────────────────────

/// Fetch releases from GitHub. When `prerelease` is false, uses the `/latest`
/// endpoint; otherwise fetches the full list and picks the newest (may include
/// pre-releases).
async fn fetch_releases(client: &reqwest::Client, prerelease: bool) -> Result<Vec<Release>> {
    if prerelease {
        // Fetch all releases (includes pre-releases).
        let url = format!("{GH_API_BASE}/repos/{GH_REPO}/releases?per_page=10");
        let resp = client
            .get(&url)
            .send()
            .await
            .context("failed to fetch releases from GitHub")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("GitHub API returned {status}: {body}");
        }

        let releases: Vec<Release> = resp
            .json()
            .await
            .context("failed to parse GitHub releases response")?;

        if releases.is_empty() {
            bail!("no releases found");
        }

        Ok(releases)
    } else {
        // Fetch only the latest release.
        let url = format!("{GH_API_BASE}/repos/{GH_REPO}/releases/latest");
        let resp = client
            .get(&url)
            .send()
            .await
            .context("failed to fetch latest release from GitHub")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("GitHub API returned {status}: {body}");
        }

        let release: Release = resp
            .json()
            .await
            .context("failed to parse latest release response")?;

        Ok(vec![release])
    }
}

// ── Version helpers ──────────────────────────────────────────────────────────

/// Find the latest release from a list. Pre-releases are included only when
/// explicitly opted in (handled by `fetch_releases`).
fn find_latest(releases: &[Release]) -> Option<&Release> {
    releases.first()
}

/// Compare two semver version strings (e.g. "0.1.0" vs "0.2.0").
/// Non-semver tags are compared lexicographically as a fallback.
fn compare_versions(current: &str, latest: &str) -> std::cmp::Ordering {
    let cur_sem = semver::Version::parse(current);
    let lat_sem = semver::Version::parse(latest);

    match (cur_sem, lat_sem) {
        (Ok(c), Ok(l)) => c.cmp(&l),
        _ => current.cmp(latest), // fallback to lexicographic
    }
}

// ── Platform detection ───────────────────────────────────────────────────────

/// Returns the expected binary asset name for the current platform, e.g.
/// `agileplus-x86_64-apple-darwin` or `agileplus-x86_64-unknown-linux-gnu`.
fn target_binary_name() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    // Normalise OS names to Rust target triples.
    let os_part = match os {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        "windows" => "pc-windows-msvc",
        other => other,
    };
    format!("agileplus-{arch}-{os_part}")
}

// ── Download & install ───────────────────────────────────────────────────────

/// Download the binary from `url` to a temporary file in `dir` and return the
/// temp file path.
async fn download_binary(
    client: &reqwest::Client,
    url: &str,
    dir: &std::path::Path,
) -> Result<PathBuf> {
    println!("Downloading {url} …");

    let resp = client
        .get(url)
        .send()
        .await
        .context("failed to download binary")?;

    if !resp.status().is_success() {
        let status = resp.status();
        bail!("download returned {status}");
    }

    let bytes = resp
        .bytes()
        .await
        .context("failed to read download response body")?;

    let tmp_path = dir.join(format!(
        ".agileplus-update-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    tokio::fs::write(&tmp_path, &bytes)
        .await
        .context("failed to write temporary binary")?;

    // Make the temp file executable (Unix only).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        tokio::fs::set_permissions(&tmp_path, perms)
            .await
            .context("failed to set executable permissions on temporary binary")?;
    }

    println!("Downloaded {} bytes.", bytes.len());
    Ok(tmp_path)
}

/// Atomically replace `target` with the binary at `source`.
fn replace_binary(source: &PathBuf, target: &PathBuf) -> Result<()> {
    // On Unix, rename is atomic if source and target are on the same filesystem
    // (which they are, since we placed the temp file next to the current exe).
    std::fs::rename(source, target)
        .or_else(|_| {
            // Fallback: copy + remove (works across filesystems on some systems).
            std::fs::copy(source, target)?;
            let _ = std::fs::remove_file(source);
            Ok(())
        })
        .with_context(|| format!("failed to replace binary at {}", target.display()))?;

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_newer() {
        assert_eq!(compare_versions("0.1.0", "0.2.0"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_versions_older() {
        assert_eq!(
            compare_versions("0.3.0", "0.2.0"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(
            compare_versions("0.1.0", "0.1.0"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_compare_versions_major() {
        assert_eq!(compare_versions("1.0.0", "2.0.0"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_versions_non_semver_fallback() {
        // Non-semver strings fall back to lexicographic comparison.
        assert_eq!(compare_versions("abc", "def"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_target_binary_name_format() {
        let name = target_binary_name();
        assert!(
            name.starts_with("agileplus-"),
            "name should start with 'agileplus-', got {name}"
        );
        // Should contain at least two hyphens: agileplus-arch-os
        assert!(
            name.matches('-').count() >= 2,
            "name should be triple: agileplus-arch-os, got {name}"
        );
    }

    #[test]
    fn test_find_latest_returns_first() {
        let releases = vec![
            Release {
                tag_name: "v0.2.0".into(),
                prerelease: false,
                assets: vec![],
            },
            Release {
                tag_name: "v0.1.0".into(),
                prerelease: false,
                assets: vec![],
            },
        ];
        let latest = find_latest(&releases).unwrap();
        assert_eq!(latest.tag_name, "v0.2.0");
    }

    #[test]
    fn test_find_latest_empty() {
        assert!(find_latest(&[]).is_none());
    }

    #[test]
    fn test_target_binary_name_contains_valid_chars() {
        let name = target_binary_name();
        // Should not contain spaces or invalid filename characters.
        assert!(!name.contains(' '));
        assert!(!name.contains('\0'));
    }
}
