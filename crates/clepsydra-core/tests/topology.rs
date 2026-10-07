use clepsydra_core::config::{Config, NodeConfig};
use clepsydra_core::new_graph;
use clepsydra_core::topology::{Behavior, Topology};
use proptest::collection::{btree_map, vec};
use proptest::prelude::*;

#[test]
fn builds_valid_linear_topology() {
    let source = r#"
        default_bucket_target = "checking"
        default_flow_source = "checking"
        default_flow_target = "savings"

        [nodes.savings]
        [nodes.checking]
        capacity = 1000
        target = "savings"
        [nodes.income]
        capacity = 5000
        target = "checking"

        [[flow]]
        volume = 500
    "#;
    let config = toml::from_str::<Config>(source);
    assert!(config.is_ok());
    if let Ok(config) = config {
        let topology = Topology::__new(&config, new_graph!());
        assert!(topology.is_ok());
        if let Ok(topology) = topology {
            assert_eq!(topology.node_count(), 3);
            assert_eq!(topology.flows().len(), 1);
            assert_eq!(topology.flows()[0].volume, 500);
            assert_eq!(
                topology.node_id("checking"),
                Some(topology.flows()[0].source)
            );
            assert_eq!(topology.node_name(topology.flows()[0].target), "savings");
        }
    }
}

#[test]
fn rejects_cycles_and_missing_targets() {
    let cycle = r#"
        [nodes.a]
        capacity = 100
        target = "b"
        [nodes.b]
        capacity = 100
        target = "a"
    "#;
    let cycle_config = toml::from_str::<Config>(cycle);
    assert!(cycle_config.is_ok());
    if let Ok(config) = cycle_config {
        assert!(Topology::__new(&config, new_graph!()).is_err());
    }

    let missing_target = r"
        [nodes.isolated]
        capacity = 500
    ";
    let missing_config = toml::from_str::<Config>(missing_target);
    assert!(missing_config.is_ok());
    if let Ok(config) = missing_config {
        assert!(Topology::__new(&config, new_graph!()).is_err());
    }
}

fn arb_node_config(name_regex: &'static str) -> impl Strategy<Value = NodeConfig> {
    (
        prop::option::of(0_u64..=9_223_372_036_854_775_807),
        prop::option::of(name_regex),
    )
        .prop_map(|(capacity, target)| NodeConfig { capacity, target })
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
        vec(
            (
                prop::option::of(name_regex),
                prop::option::of(name_regex),
                0_u64..=9_223_372_036_854_775_807,
            )
                .prop_map(|(source, target, volume)| {
                    clepsydra_core::config::FlowConfig {
                        source,
                        target,
                        volume,
                    }
                }),
            0..=max_flows,
        ),
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000))]

    #[test]
    fn construction_is_deterministic(config in arb_config("[a-d]{1,4}", 20, 10)) {
        let first = Topology::__new(&config, new_graph!());
        let second = Topology::__new(&config, new_graph!());
        match (first, second) {
            (Ok(first), Ok(second)) => {
                let mut first_names: Vec<_> = first.graph().iter().map(|(id, _)| first.node_name(id)).collect();
                let mut second_names: Vec<_> = second.graph().iter().map(|(id, _)| second.node_name(id)).collect();
                first_names.sort_unstable();
                second_names.sort_unstable();
                prop_assert_eq!(first_names, second_names);
            }
            (Err(first), Err(second)) => prop_assert_eq!(first, second),
            _ => prop_assert!(false, "equivalent configurations produced different outcomes"),
        }
    }

    #[test]
    fn graph_maps_match(config in arb_config("[a-z]{1,5}", 50, 0)) {
        if let Ok(topology) = Topology::__new(&config, new_graph!()) {
            let nodes: Vec<_> = topology.graph().iter().collect();
            prop_assert_eq!(nodes.len(), topology.node_count());
            for (_id, node) in nodes {
                if let Behavior::Bucket { target, .. } = &node.behavior {
                    prop_assert_eq!(topology.node_name(*target), topology.graph().get(*target).name());
                }
            }
        }
    }
}
