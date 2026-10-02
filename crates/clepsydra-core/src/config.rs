use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
