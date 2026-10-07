use clepsydra_core::config::{Config, FlowConfig, NodeConfig};
use clepsydra_core::errors::EvalError;
use clepsydra_core::runtime::{
    RuntimeEvaluator, clepsydra, clepsydra_eval, clepsydra_map_eval, clepsydra_vec_eval,
};
use std::collections::{BTreeMap, HashMap};

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
    let result =
        clepsydra(&config()).and_then(|evaluator| evaluator.eval([("sink", 0), ("checking", 7)]));
    assert_eq!(result, Ok([("sink", 3), ("checking", 4)]));
}

#[test]
fn evaluates_fixed_array_in_caller_order() {
    let result = clepsydra(&config()).and_then(|evaluator| evaluator.ordered_eval([0, 7]));
    assert_eq!(result, Ok([3, 4]));
}

#[test]
fn config_helpers_match_runtime_evaluator_shapes() {
    assert_eq!(
        clepsydra(&config()).and_then(|evaluator| evaluator.eval([("checking", 7), ("sink", 0)])),
        Ok([("checking", 4), ("sink", 3)])
    );
    assert_eq!(
        clepsydra(&config()).and_then(|evaluator| {
            evaluator.map_eval(HashMap::from([
                ("checking".to_owned(), 7),
                ("sink".to_owned(), 0),
            ]))
        }),
        Ok(HashMap::from([
            ("checking".to_owned(), 4),
            ("sink".to_owned(), 3)
        ]))
    );
}

#[test]
fn evaluates_with_macro_shaped_runtime_api() {
    let runtime_config = config();
    let evaluator = clepsydra(&runtime_config);

    assert_eq!(
        evaluator
            .as_ref()
            .map(|evaluator| evaluator.ordered_eval([0, 7])),
        Ok(Ok([3, 4]))
    );
    assert_eq!(
        evaluator
            .as_ref()
            .map(|evaluator| evaluator.eval([("checking", 7), ("sink", 0)])),
        Ok(Ok([("checking", 4), ("sink", 3)]))
    );

    let state = HashMap::from([("checking".to_owned(), 7), ("sink".to_owned(), 0)]);
    let expected = HashMap::from([("checking".to_owned(), 4), ("sink".to_owned(), 3)]);
    assert_eq!(
        evaluator.map(|evaluator| evaluator.map_eval(state)),
        Ok(Ok(expected))
    );
}

#[test]
fn runtime_constructor_variants_share_the_same_struct_api() {
    let evaluator = clepsydra(&config());
    assert_eq!(evaluator.as_ref().map(RuntimeEvaluator::node_count), Ok(2));
    assert_eq!(
        evaluator.as_ref().map(RuntimeEvaluator::names),
        Ok(["sink".to_owned(), "checking".to_owned()].as_slice())
    );
    let Ok(named) = clepsydra_eval(&config()) else {
        return;
    };
    assert_eq!(
        named(&[("checking", 7), ("sink", 0)]),
        Ok(vec![("checking", 4), ("sink", 3)])
    );
    let Ok(vec_named) = clepsydra_vec_eval(&config()) else {
        return;
    };
    assert_eq!(
        vec_named(&[("checking", 7), ("sink", 0)]),
        Ok(vec![("checking", 4), ("sink", 3)])
    );
    let Ok(dictionary) = clepsydra_map_eval(&config()) else {
        return;
    };
    assert_eq!(
        dictionary(HashMap::from([
            ("checking".to_owned(), 7),
            ("sink".to_owned(), 0),
        ])),
        Ok(HashMap::from([
            ("checking".to_owned(), 4),
            ("sink".to_owned(), 3),
        ]))
    );
}

#[test]
fn propagates_bucket_overflow_to_sink() {
    let mut overflow_config = config();
    overflow_config.flow = vec![FlowConfig {
        source: Some("sink".to_owned()),
        target: Some("checking".to_owned()),
        volume: 5,
    }];

    let result = clepsydra(&overflow_config)
        .and_then(|evaluator| evaluator.eval([("checking", 9), ("sink", 5)]));
    assert_eq!(result, Ok([("checking", 10), ("sink", 4)]));
}

#[test]
fn rejects_invalid_runtime_inputs() {
    let evaluator = clepsydra(&config());

    assert_eq!(
        evaluator
            .as_ref()
            .map(|evaluator| evaluator.eval([("checking", 7)])),
        Ok(Err(EvalError::InvalidInputCount {
            expected: 2,
            actual: 1,
        }))
    );
    assert_eq!(
        clepsydra(&config())
            .and_then(|evaluator| evaluator.eval([("checking", 7), ("checking", 0)])),
        Err(EvalError::DuplicateInput {
            name: "checking".to_owned(),
        })
    );
    assert_eq!(
        clepsydra(&config())
            .and_then(|evaluator| evaluator.eval([("checking", 7), ("unknown", 0)])),
        Err(EvalError::UnknownInput {
            name: "unknown".to_owned(),
        })
    );
}

#[test]
fn rejects_fixed_array_with_wrong_node_count() {
    let result = clepsydra(&config()).and_then(|evaluator| evaluator.ordered_eval([7]));
    assert_eq!(
        result,
        Err(EvalError::InvalidInputCount {
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
    let result = clepsydra(&invalid).map(|_| ());
    assert_eq!(
        result,
        Err(EvalError::Topology(
            clepsydra_core::topology::TopologyError::MissingTarget {
                node: "checking".to_owned(),
            }
        ))
    );
}
