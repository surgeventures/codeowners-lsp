use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::diagnostics::DiagnosticConfig;

pub const CONFIG_FILE: &str = ".codeowners-lsp.toml";
pub const CONFIG_FILE_LOCAL: &str = ".codeowners-lsp.local.toml";

/// Settings for the suggest command
#[derive(Debug, Default, Deserialize, Clone)]
pub struct SuggestSettings {
    /// Command to lookup team from email
    /// Use {email} as placeholder, e.g.: "your-tool lookup {email} | jq -r .team"
    pub lookup_cmd: Option<String>,
    /// Prepend / to paths (anchored patterns)
    #[serde(default)]
    pub anchored: bool,
}

/// Shared settings for both LSP and CLI
#[derive(Debug, Default, Deserialize, Clone)]
pub struct Settings {
    /// Custom path to CODEOWNERS file (relative to workspace root)
    pub path: Option<String>,
    /// Individual owner identifier (e.g. @username)
    pub individual: Option<String>,
    /// Team owner identifier (e.g. @org/team-name)
    pub team: Option<String>,
    /// GitHub token for validating owners (reads from env if prefixed with "env:")
    pub github_token: Option<String>,
    /// Whether to validate owners against GitHub API
    #[serde(default)]
    pub validate_owners: bool,
    /// Diagnostic severity overrides (code -> "off"|"hint"|"info"|"warning"|"error")
    #[serde(default)]
    pub diagnostics: HashMap<String, String>,
    /// Suggest command settings
    #[serde(default)]
    pub suggest: SuggestSettings,
}

impl Settings {
    /// Merge another Settings into this one (other takes precedence for set values)
    pub fn merge(&mut self, other: Settings) {
        if other.path.is_some() {
            self.path = other.path;
        }
        if other.individual.is_some() {
            self.individual = other.individual;
        }
        if other.team.is_some() {
            self.team = other.team;
        }
        if other.github_token.is_some() {
            self.github_token = other.github_token;
        }
        if other.validate_owners {
            self.validate_owners = true;
        }
        // Merge diagnostics (other overwrites same keys)
        for (k, v) in other.diagnostics {
            self.diagnostics.insert(k, v);
        }
        // Merge suggest settings
        if other.suggest.lookup_cmd.is_some() {
            self.suggest.lookup_cmd = other.suggest.lookup_cmd;
        }
        if other.suggest.anchored {
            self.suggest.anchored = true;
        }
    }

    /// Get DiagnosticConfig from settings
    pub fn diagnostic_config(&self) -> DiagnosticConfig {
        DiagnosticConfig::from_map(&self.diagnostics)
    }

    /// Resolve GitHub token (handles env: prefix)
    pub fn resolve_token(&self) -> Option<String> {
        self.github_token.as_ref().and_then(|token| {
            if let Some(env_var) = token.strip_prefix("env:") {
                std::env::var(env_var).ok()
            } else {
                Some(token.clone())
            }
        })
    }

    /// Get the lookup command template if configured
    #[allow(dead_code)] // Used by CLI only
    pub fn lookup_cmd(&self) -> Option<&str> {
        self.suggest.lookup_cmd.as_deref()
    }

    /// Get whether to use anchored paths in suggestions
    #[allow(dead_code)] // Used by CLI only
    pub fn suggest_anchored(&self) -> bool {
        self.suggest.anchored
    }
}

/// Load settings from config files in a directory
/// Priority: defaults < .codeowners-lsp.toml < .codeowners-lsp.local.toml
pub fn load_settings_from_path(root: &Path) -> Settings {
    let mut settings = Settings::default();

    // Load project config
    let config_path = root.join(CONFIG_FILE);
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(file_settings) = toml::from_str::<Settings>(&content) {
            settings.merge(file_settings);
        }
    }

    // Load local config (user overrides)
    let local_config_path = root.join(CONFIG_FILE_LOCAL);
    if let Ok(content) = fs::read_to_string(&local_config_path) {
        if let Ok(file_settings) = toml::from_str::<Settings>(&content) {
            settings.merge(file_settings);
        }
    }

    settings
}

/// Load settings from current working directory
#[allow(dead_code)] // Used by CLI only
pub fn load_settings() -> Settings {
    let cwd = std::env::current_dir().unwrap_or_default();
    load_settings_from_path(&cwd)
}
