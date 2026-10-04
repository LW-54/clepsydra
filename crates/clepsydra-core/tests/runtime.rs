use clepsydra_core::config::{Config, FlowConfig, NodeConfig};
use clepsydra_core::new_graph;
use clepsydra_core::runtime::{
    RuntimeError, RuntimeEvaluator, evaluate_config, evaluate_config_array,
};
use clepsydra_core::symbolic::SymbolicState;
use clepsydra_core::topology::Topology;
use std::collections::BTreeMap;

fn config() -> Config {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "sink".to_owned(),
        NodeConfig {
            capacity: None,
            target: None,
        },
    );
    nodes.insert(
        "checking".to_owned(),
        NodeConfig {
            capacity: Some(10),
            target: Some("sink".to_owned()),
        },
    );
    Config {
        default_bucket_target: None,
        default_flow_source: None,
        default_flow_target: None,
        nodes,
        flow: vec![FlowConfig {
            source: Some("checking".to_owned()),
            target: Some("sink".to_owned()),
            volume: 3,
        }],
    }
}

#[test]
fn evaluates_dynamic_state_in_name_order() {
    let result = evaluate_config(&config(), &[("sink", 0), ("checking", 7)]);
    assert_eq!(
        result,
        Ok(vec![("checking".to_owned(), 4), ("sink".to_owned(), 3)])
    );
}

#[test]
fn evaluates_fixed_array_in_caller_order() {
    let result = evaluate_config_array(&config(), [("sink", 0), ("checking", 7)]);
    assert_eq!(result, Ok([("sink", 3), ("checking", 4)]));
}

#[test]
fn propagates_bucket_overflow_to_sink() {
    let mut overflow_config = config();
    overflow_config.flow = vec![FlowConfig {
        source: Some("sink".to_owned()),
        target: Some("checking".to_owned()),
        volume: 5,
    }];

    let result = evaluate_config(&overflow_config, &[("checking", 9), ("sink", 5)]);
    assert_eq!(
        result,
        Ok(vec![("checking".to_owned(), 10), ("sink".to_owned(), 4)])
    );
}

#[test]
fn rejects_invalid_runtime_inputs() {
    let graph = new_graph!();
    let topology = Topology::__new(&config(), graph);
    assert!(topology.is_ok());
    let Ok(topology) = topology else { return };
    let symbolic = SymbolicState::new(&topology);
    let evaluator = RuntimeEvaluator::new(&symbolic);

    assert_eq!(
        evaluator.evaluate(&[("checking", 7)]),
        Err(RuntimeError::InvalidInputCount {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        evaluator.evaluate(&[("checking", 7), ("checking", 0)]),
        Err(RuntimeError::DuplicateInput {
            name: "checking".to_owned(),
        })
    );
    assert_eq!(
        evaluator.evaluate(&[("checking", 7), ("unknown", 0)]),
        Err(RuntimeError::UnknownInput {
            name: "unknown".to_owned(),
        })
    );
}

#[test]
fn rejects_fixed_array_with_wrong_node_count() {
    let result = evaluate_config_array(&config(), [("checking", 7)]);
    assert_eq!(
        result,
        Err(RuntimeError::InvalidInputCount {
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn reports_topology_errors_from_config_helper() {
    let mut invalid = config();
    if let Some(node) = invalid.nodes.get_mut("checking") {
        node.target = None;
    }
    let result = evaluate_config(&invalid, &[("checking", 0), ("sink", 0)]);
    assert_eq!(
        result,
        Err(RuntimeError::Topology(
            clepsydra_core::topology::TopologyError::MissingTarget {
                node: "checking".to_owned(),
            }
        ))
    );
}
