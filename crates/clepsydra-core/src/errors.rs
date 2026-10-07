use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug, PartialEq, Eq)]
pub enum TopologyError {
    MissingCapacity { node: String },
    MissingTarget { node: String },
    UnresolvedTargets { nodes: Vec<String> },
    MissingFlowSource { flow: usize },
    MissingFlowTarget { flow: usize },
    UnknownFlowSource { flow: usize, node: String },
    UnknownFlowTarget { flow: usize, node: String },
}

impl Display for TopologyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCapacity { node } => {
                write!(formatter, "Node '{node}' has a target but no capacity")
            }
            Self::MissingTarget { node } => write!(
                formatter,
                "Node '{node}' has capacity but no target (and no default_bucket_target exists)"
            ),
            Self::UnresolvedTargets { nodes } => write!(
                formatter,
                "Cycle detected or missing target node involving: {nodes:?}"
            ),
            Self::MissingFlowSource { .. } => write!(formatter, "Missing Flow source"),
            Self::MissingFlowTarget { .. } => write!(formatter, "Missing Flow target"),
            Self::UnknownFlowSource { node, .. } => {
                write!(formatter, "Flow source '{node}' not found")
            }
            Self::UnknownFlowTarget { node, .. } => {
                write!(formatter, "Flow target '{node}' not found")
            }
        }
    }
}

impl std::error::Error for TopologyError {}

#[derive(Debug, PartialEq, Eq)]
pub enum EvalError {
    DuplicateInput { name: String },
    UnknownInput { name: String },
    MissingInput { name: String },
    InvalidInputCount { expected: usize, actual: usize },
    Topology(TopologyError),
}

impl Display for EvalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateInput { name } => write!(formatter, "Duplicate input '{name}'"),
            Self::UnknownInput { name } => write!(formatter, "Unknown input '{name}'"),
            Self::MissingInput { name } => write!(formatter, "Missing input '{name}'"),
            Self::InvalidInputCount { expected, actual } => write!(
                formatter,
                "Expected {expected} inputs, but received {actual}",
            ),
            Self::Topology(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for EvalError {}

impl From<TopologyError> for EvalError {
    fn from(error: TopologyError) -> Self {
        Self::Topology(error)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: String,
        source: io::Error,
    },
    NotAFile {
        path: String,
    },
    UnsupportedFormat {
        path: String,
        extension: Option<String>,
    },
    Parse {
        source: String,
        message: String,
    },
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "Failed to read config '{path}': {source}")
            }
            Self::NotAFile { path } => write!(formatter, "Config path '{path}' is not a file"),
            Self::UnsupportedFormat { path, extension } => match extension {
                Some(extension) => write!(
                    formatter,
                    "Unsupported config format '.{extension}' for '{path}'"
                ),
                None => write!(
                    formatter,
                    "Config path '{path}' has no supported file extension"
                ),
            },
            Self::Parse { source, message } => {
                write!(formatter, "Failed to parse config from {source}: {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}
