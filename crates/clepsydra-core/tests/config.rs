use clepsydra_core::config::{Config, FlowConfig, NodeConfig};
use clepsydra_core::errors::ConfigError;
use proptest::collection::{btree_map, vec};
use proptest::prelude::*;

fn arb_node_config(name_regex: &'static str) -> impl Strategy<Value = NodeConfig> {
    (
        prop::option::of(0_u64..=9_223_372_036_854_775_807),
        prop::option::of(name_regex),
    )
        .prop_map(|(capacity, target)| NodeConfig { capacity, target })
}

fn arb_flow_config(name_regex: &'static str) -> impl Strategy<Value = FlowConfig> {
    (
        prop::option::of(name_regex),
        prop::option::of(name_regex),
        0_u64..=9_223_372_036_854_775_807,
    )
        .prop_map(|(source, target, volume)| FlowConfig {
            source,
            target,
            volume,
        })
}

fn arb_config(
    name_regex: &'static str,
    max_nodes: usize,
    max_flows: usize,
) -> impl Strategy<Value = Config> {
    (
        prop::option::of(name_regex),
        prop::option::of(name_regex),
        prop::option::of(name_regex),
        btree_map(name_regex, arb_node_config(name_regex), 0..=max_nodes),
        vec(arb_flow_config(name_regex), 0..=max_flows),
    )
        .prop_map(
            |(default_bucket_target, default_flow_source, default_flow_target, nodes, flow)| {
                Config {
                    default_bucket_target,
                    default_flow_source,
                    default_flow_target,
                    nodes,
                    flow,
                }
            },
        )
}

#[test]
fn checked_in_schema_is_current() {
    let schema = schemars::schema_for!(Config);
    let generated = serde_json::to_string_pretty(&schema);
    assert!(generated.is_ok());
    let checked_in = std::fs::read_to_string("../../clepsydra.schema.json");
    assert!(checked_in.is_ok());
    assert_eq!(generated.ok(), checked_in.ok());
}

#[test]
fn rejects_invalid_toml_types_and_fields() {
    assert!(toml::from_str::<Config>("[nodes.a]\ncapacity = -50").is_err());
    assert!(toml::from_str::<Config>("[nodes.a]\ncapacity = 10.5").is_err());
    assert!(toml::from_str::<Config>("[nodes.a]\ncapaciy = 100").is_err());
    assert!(toml::from_str::<Config>("[nodes.a]\ndefault_bucket_target = 'b'").is_err());
}

#[test]
fn accepts_banking_aliases_in_supported_formats() {
    fn assert_banking_config<E: std::fmt::Display>(parsed: Result<Config, E>) {
        assert!(parsed.is_ok(), "failed to parse banking aliases");
        if let Ok(config) = parsed {
            assert_eq!(config.default_bucket_target.as_deref(), Some("checking"));
            assert_eq!(config.default_flow_source.as_deref(), Some("checking"));
            assert_eq!(config.default_flow_target.as_deref(), Some("checking"));
            assert!(config.nodes.contains_key("checking"));
            assert_eq!(config.flow[0].volume, 3);
        }
    }

    let toml_source = r#"
        default_account_target = "checking"
        default_transfer_source = "checking"
        default_transfer_target = "checking"

        [accounts.checking]
        capacity = 10

        [[transfers]]
        source = "checking"
        target = "checking"
        amount = 3
    "#;
    let json_source = r#"{
        "default_account_target": "checking",
        "default_transfer_source": "checking",
        "default_transfer_target": "checking",
        "accounts": {"checking": {"capacity": 10}},
        "transfers": [{"source": "checking", "target": "checking", "amount": 3}]
    }"#;
    let yaml_source = r"
        default_account_target: checking
        default_transfer_source: checking
        default_transfer_target: checking
        accounts:
          checking:
            capacity: 10
        transfers:
          - source: checking
            target: checking
            amount: 3
    ";

    assert_banking_config(toml::from_str(toml_source));
    assert_banking_config(serde_json::from_str(json_source));
    assert_banking_config(serde_yaml::from_str(yaml_source));
}

#[test]
fn config_from_input_accepts_supported_inline_formats() {
    let toml = "[nodes.sink]";
    let json = r#"{"nodes":{"sink":{"capacity":null,"target":null}}}"#;
    let yaml = "nodes:\n  sink:\n    capacity: null\n    target: null\n";

    assert!(Config::from_input(toml).is_ok());
    assert!(Config::from_input(json).is_ok());
    assert!(Config::from_input(yaml).is_ok());
}

#[test]
fn config_from_input_reports_parse_errors() {
    let result = Config::from_input("not valid config content");
    assert!(matches!(result, Err(ConfigError::Parse { .. })));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000))]

    #[test]
    fn config_roundtrips_through_toml(original in arb_config("[a-z]{1,5}", 20, 10)) {
        let toml_string = toml::to_string(&original);
        prop_assert!(toml_string.is_ok());
        if let Ok(toml_string) = toml_string {
            let parsed = toml::from_str::<Config>(&toml_string);
            prop_assert_eq!(parsed, Ok(original));
        }
    }
}
