use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_ORIGIN: &str = "https://mob.so";

/// One stored login. `kind` records what /auth/me reported when the context
/// was created; every request still authorizes server side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredContext {
    pub token: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub handle: String,
    #[serde(default = "default_origin")]
    pub origin: String,
}

fn default_origin() -> String {
    DEFAULT_ORIGIN.to_string()
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub contexts: BTreeMap<String, StoredContext>,
}

pub fn mob_home() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("MOB_HOME") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("Could not resolve a home directory (HOME or USERPROFILE)")?;
    Ok(PathBuf::from(home).join(".mob"))
}

fn config_path() -> Result<PathBuf> {
    Ok(mob_home()?.join("config.json"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("Could not read {}", path.display()))?;
    let config = serde_json::from_str(&raw)
        .with_context(|| format!("Could not parse {}", path.display()))?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(config)?;
    fs::write(&path, raw).with_context(|| format!("Could not write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// The origin and token the current invocation runs under.
///
/// Precedence: MOB_TOKEN / MOB_API_URL environment variables, then the
/// selected stored context. A command that needs no credential (public
/// reads, login itself) tolerates a missing context.
pub struct Session {
    pub origin: String,
    pub token: Option<String>,
    pub context_name: Option<String>,
}

pub fn session() -> Result<Session> {
    let env_token = std::env::var("MOB_TOKEN").ok().filter(|t| !t.is_empty());
    let env_origin = std::env::var("MOB_API_URL").ok().filter(|o| !o.is_empty());

    let config = load()?;
    let stored = if config.active.is_empty() {
        None
    } else {
        config
            .contexts
            .get(&config.active)
            .map(|context| (config.active.clone(), context.clone()))
    };

    let origin = env_origin
        .or_else(|| stored.as_ref().map(|(_, c)| c.origin.clone()))
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string());
    let origin = origin.trim_end_matches('/').to_string();

    if let Some(token) = env_token {
        return Ok(Session {
            origin,
            token: Some(token),
            context_name: None,
        });
    }

    match stored {
        Some((name, context)) => Ok(Session {
            origin,
            token: Some(context.token),
            context_name: Some(name),
        }),
        None => Ok(Session {
            origin,
            token: None,
            context_name: None,
        }),
    }
}

pub fn require_session() -> Result<Session> {
    let session = session()?;
    if session.token.is_none() {
        bail!("Not logged in. Run `mob login`, or set MOB_TOKEN.");
    }
    Ok(session)
}
