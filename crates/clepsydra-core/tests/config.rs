use clepsydra_core::config::{Config, FlowConfig, NodeConfig};
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
fn schema_is_generated() {
    let schema = schemars::schema_for!(Config);
    let schema_json = serde_json::to_string_pretty(&schema);
    assert!(schema_json.is_ok());
    assert!(
        std::fs::write(
            "../../clepsydra.schema.json",
            schema_json.unwrap_or_default()
        )
        .is_ok()
    );
}

#[test]
fn rejects_invalid_toml_types_and_fields() {
    assert!(toml::from_str::<Config>("[nodes.a]\ncapacity = -50").is_err());
    assert!(toml::from_str::<Config>("[nodes.a]\ncapacity = 10.5").is_err());
    assert!(toml::from_str::<Config>("[nodes.a]\ncapaciy = 100").is_err());
    assert!(toml::from_str::<Config>("[nodes.a]\ndefault_bucket_target = 'b'").is_err());
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
