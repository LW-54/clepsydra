use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub default_bucket_target: Option<String>,
    pub default_flow_source: Option<String>,
    pub default_flow_target: Option<String>,

    pub nodes: std::collections::BTreeMap<String, NodeConfig>,

    #[serde(default)]
    pub flow: Vec<FlowConfig>,
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
    pub volume: u64,
}
