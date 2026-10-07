use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::errors::ConfigError;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(alias = "default_account_target")]
    pub default_bucket_target: Option<String>,
    #[serde(alias = "default_transfer_source")]
    pub default_flow_source: Option<String>,
    #[serde(alias = "default_transfer_target")]
    pub default_flow_target: Option<String>,

    #[serde(alias = "accounts")]
    pub nodes: std::collections::BTreeMap<String, NodeConfig>,

    #[serde(default)]
    #[serde(alias = "transfers")]
    pub flow: Vec<FlowConfig>,
}

impl Config {
    /// Parses inline TOML, JSON, or YAML, or reads an existing file path.
    ///
    /// # Errors
    ///
    /// Returns a read, path, or parse error.
    pub fn from_input(input: &str) -> Result<Self, ConfigError> {
        let trimmed = input.trim();
        let path = Path::new(trimmed);
        if !trimmed.contains(['\n', '\r']) && path.exists() {
            return Self::from_path(path);
        }
        Self::parse_text(trimmed, "inline input")
    }

    /// Reads and parses a TOML, JSON, YAML, or YML file.
    ///
    /// # Errors
    ///
    /// Returns a read, path, format, or parse error.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let label = path.display().to_string();
        if !path.is_file() {
            return Err(ConfigError::NotAFile { path: label });
        }
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        let format = extension
            .as_deref()
            .filter(|extension| matches!(*extension, "toml" | "json" | "yaml" | "yml"));
        let Some(format) = format else {
            return Err(ConfigError::UnsupportedFormat {
                path: label,
                extension,
            });
        };
        let source = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: label.clone(),
            source,
        })?;
        Self::parse_text_with_format(&source, &label, format)
    }

    fn parse_text(source: &str, label: &str) -> Result<Self, ConfigError> {
        toml::from_str(source)
            .or_else(|_| serde_json::from_str(source))
            .or_else(|_| serde_yaml::from_str(source))
            .map_err(|error| ConfigError::Parse {
                source: label.to_owned(),
                message: error.to_string(),
            })
    }

    fn parse_text_with_format(
        source: &str,
        label: &str,
        format: &str,
    ) -> Result<Self, ConfigError> {
        let result = match format {
            "toml" => toml::from_str(source).map_err(|error| error.to_string()),
            "json" => serde_json::from_str(source).map_err(|error| error.to_string()),
            "yaml" | "yml" => serde_yaml::from_str(source).map_err(|error| error.to_string()),
            _ => {
                return Err(ConfigError::UnsupportedFormat {
                    path: label.to_owned(),
                    extension: Some(format.to_owned()),
                });
            }
        };
        result.map_err(|error| ConfigError::Parse {
            source: label.to_owned(),
            message: error,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
    pub capacity: Option<u64>,
    pub target: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowConfig {
    pub source: Option<String>,
    pub target: Option<String>,
    #[serde(alias = "amount")]
    pub volume: u64,
}
