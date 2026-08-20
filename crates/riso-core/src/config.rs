//! The user's configuration file.
//!
//! Deliberately tiny: the handful of choices that change what riso does by
//! default, each one also reachable another way. Everything situational
//! stays a command-line flag, documented in riso(1).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::IoError;

/// The file's name under the riso config directory.
const FILE: &str = "config.toml";

/// Everything the file can say. Unknown keys are rejected, so a typo fails
/// loudly instead of silently meaning the default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// Whether Omarchy's theme directories join the default search path.
    pub omarchy_themes: bool,
    /// Output format when `-o` is not given: `human`, `json` or `yaml`.
    pub output: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            omarchy_themes: true,
            output: "human".into(),
        }
    }
}

/// Where the file lives: `$XDG_CONFIG_HOME/riso/config.toml`.
pub fn path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .map(|base| base.join("riso").join(FILE))
}

impl Config {
    /// Read the file, or the defaults when there is none.
    pub fn load() -> Result<Self, String> {
        let Some(path) = path() else {
            return Ok(Self::default());
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => return Err(format!("failed to read {}: {e}", path.display())),
        };
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The defaults when the file is missing or unreadable. Broken
    /// configuration must never take the rest of riso down with it; the
    /// `config` command is where errors are surfaced instead.
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }

    /// Write the file, creating it if needed. Returns where it was written.
    pub fn save(&self) -> Result<PathBuf, IoError> {
        let path = path().ok_or_else(|| IoError::NoParent(PathBuf::from(FILE)))?;
        let body = toml::to_string(self).expect("config serializes to TOML");
        let text = format!("# riso options; the full reference is riso(1) and the README.\n{body}");
        crate::atomic::write_atomic(&path, &text)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_omarchy_and_human_output() {
        let config = Config::default();
        assert!(config.omarchy_themes);
        assert_eq!(config.output, "human");
    }

    #[test]
    fn a_partial_file_fills_in_defaults() {
        let config: Config = toml::from_str("omarchy-themes = false").unwrap();
        assert!(!config.omarchy_themes);
        assert_eq!(config.output, "human");
    }

    #[test]
    fn an_unknown_key_is_an_error() {
        assert!(toml::from_str::<Config>("omarchy = false").is_err());
    }

    #[test]
    fn round_trips_through_toml() {
        let config = Config {
            omarchy_themes: false,
            output: "json".into(),
        };
        let text = toml::to_string(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
    }
}
