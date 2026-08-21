//! The upgrade pathway, following the railway CLI's shape: detect the
//! install method from the executable path, compare the running version
//! with the latest GitHub release through a daily cache, and upgrade
//! through the channel that installed the binary.

use std::cmp::Ordering;
use std::io::IsTerminal;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::config::mob_home;

const RELEASE_API_URL: &str = "https://api.github.com/repos/mobdotso/cli/releases/latest";
const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    Homebrew,
    Npm,
    Cargo,
    Shell,
    Scoop,
    Unknown,
}

impl InstallMethod {
    /// Classifies the running binary by its resolved path, checking package
    /// managers before the shell-install catch-all.
    pub fn detect() -> Self {
        let Ok(exe_path) = std::env::current_exe() else {
            return InstallMethod::Unknown;
        };
        let exe_path = exe_path.canonicalize().unwrap_or(exe_path);
        let path = exe_path.to_string_lossy().to_lowercase();

        if path.contains("homebrew") || path.contains("cellar") || path.contains("linuxbrew") {
            return InstallMethod::Homebrew;
        }
        // pnpm paths contain "npm" as a substring; the CLI has no pnpm
        // upgrade command, so classify them as unknown.
        if path.contains("pnpm") {
            return InstallMethod::Unknown;
        }
        if path.contains("node_modules") || path.contains("npm") {
            return InstallMethod::Npm;
        }
        if path.contains(".cargo") && path.contains("bin") {
            return InstallMethod::Cargo;
        }
        if path.contains("scoop") {
            return InstallMethod::Scoop;
        }
        // Paths owned by system package managers stay unknown so the CLI
        // never runs an installer against a package it does not manage.
        const SYSTEM_PATHS: &[&str] = &["/usr/bin", "/usr/sbin", "/nix/", "/snap/", "/flatpak/"];
        if SYSTEM_PATHS.iter().any(|p| path.contains(p)) {
            return InstallMethod::Unknown;
        }
        if exe_path
            .parent()
            .and_then(|dir| dir.file_name())
            .map(|name| name == "bin")
            .unwrap_or(false)
        {
            return InstallMethod::Shell;
        }
        InstallMethod::Unknown
    }

    pub fn name(&self) -> &'static str {
        match self {
            InstallMethod::Homebrew => "Homebrew",
            InstallMethod::Npm => "npm",
            InstallMethod::Cargo => "Cargo",
            InstallMethod::Shell => "install script",
            InstallMethod::Scoop => "Scoop",
            InstallMethod::Unknown => "unknown",
        }
    }

    /// The program and arguments that upgrade this install.
    pub fn upgrade_command(&self) -> Option<(&'static str, Vec<&'static str>)> {
        match self {
            InstallMethod::Homebrew => Some(("brew", vec!["upgrade", "mobdotso/tap/mobs"])),
            InstallMethod::Npm => Some(("npm", vec!["install", "-g", "@mobdotso/mobs@latest"])),
            InstallMethod::Cargo => Some((
                "cargo",
                vec!["install", "--git", "https://github.com/mobdotso/cli", "--force"],
            )),
            InstallMethod::Scoop => Some(("scoop", vec!["update", "mobs"])),
            InstallMethod::Shell => Some((
                "sh",
                vec!["-c", "curl -fsSL https://mob.so/install.sh | sh -s -- --yes"],
            )),
            InstallMethod::Unknown => None,
        }
    }
}

/// Compares two x.y.z version strings numerically; a malformed segment
/// counts as zero.
fn compare_versions(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(a), parse(b));
    for i in 0..a.len().max(b.len()) {
        let ord = a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0));
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct UpdateCache {
    #[serde(default)]
    last_checked: u64,
    #[serde(default)]
    latest: Option<String>,
}

impl UpdateCache {
    fn path() -> Result<std::path::PathBuf> {
        Ok(mob_home()?.join("version.json"))
    }

    fn read() -> Self {
        Self::path()
            .ok()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn write(&self) {
        let Ok(path) = Self::path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, raw);
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// Fetches the latest release tag from GitHub.
fn fetch_latest(timeout: Duration) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent(format!("mobs/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    let release: Release = client
        .get(RELEASE_API_URL)
        .send()
        .context("Could not reach the GitHub release API")?
        .error_for_status()
        .context("The GitHub release API answered an error")?
        .json()
        .context("Could not parse the GitHub release response")?;
    Ok(release.tag_name.trim_start_matches('v').to_string())
}

/// Prints a notice on stderr when a newer release exists. The check runs
/// against a daily cache in ~/.mob/version.json; a network failure passes
/// silently. Skipped when stderr is not a terminal.
pub fn notify_if_outdated() {
    if !std::io::stderr().is_terminal() {
        return;
    }
    let current = env!("CARGO_PKG_VERSION");
    let mut cache = UpdateCache::read();

    if now_secs().saturating_sub(cache.last_checked) >= CHECK_INTERVAL_SECS {
        cache.last_checked = now_secs();
        cache.latest = fetch_latest(Duration::from_secs(2)).ok();
        cache.write();
    }

    if let Some(latest) = &cache.latest {
        if compare_versions(current, latest) == Ordering::Less {
            eprintln!(
                "\nmobs {latest} is available (you have {current}). Run {} to update.",
                "mobs upgrade".bold()
            );
        }
    }
}

/// The `mobs upgrade` command.
pub fn run(check: bool) -> Result<()> {
    let method = InstallMethod::detect();
    let current = env!("CARGO_PKG_VERSION");

    if check {
        println!("version: {current}");
        println!("install method: {}", method.name());
        if let Ok(path) = std::env::current_exe() {
            println!("binary: {}", path.display());
        }
        if let Some((program, args)) = method.upgrade_command() {
            println!("upgrade command: {} {}", program, args.join(" "));
        }
        match fetch_latest(Duration::from_secs(10)) {
            Ok(latest) => println!("latest release: {latest}"),
            Err(error) => println!("latest release: unavailable ({error:#})"),
        }
        return Ok(());
    }

    let latest = fetch_latest(Duration::from_secs(10))?;
    if compare_versions(current, &latest) != Ordering::Less {
        println!("mobs {current} is the latest release.");
        return Ok(());
    }

    println!(
        "Upgrading mobs {current} to {latest} via {}.",
        method.name()
    );

    let Some((program, args)) = method.upgrade_command() else {
        println!("The install method could not be detected. Upgrade through the channel you installed with:");
        println!("  install script:  curl -fsSL https://mob.so/install.sh | sh");
        println!("  Homebrew:        brew upgrade mobdotso/tap/mobs");
        println!("  npm:             npm install -g @mobdotso/mobs@latest");
        println!("  Scoop:           scoop update mobs");
        println!("  Cargo:           cargo install --git https://github.com/mobdotso/cli --force");
        return Ok(());
    };

    println!("Running: {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(&args)
        .status()
        .with_context(|| format!("Could not run {program}"))?;
    if !status.success() {
        bail!(
            "The upgrade command failed with exit code {}",
            status.code().unwrap_or(-1)
        );
    }

    UpdateCache {
        last_checked: now_secs(),
        latest: None,
    }
    .write();

    println!("  {} mobs {latest} installed.", "✓".green());
    Ok(())
}
