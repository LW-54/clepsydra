use clepsydra_lib::clepsydra;
use clepsydra_lib::clepsydra_core::config::{Config, NodeConfig};
use clepsydra_lib::clepsydra_eval;
use std::collections::BTreeMap;

#[test]
fn facade_reexports_eval_macro() {
    let evaluate = clepsydra_eval!(
        r#"
        [nodes.sink]

        [nodes.checking]
        capacity = 10
        target = "sink"

        [[flow]]
        source = "checking"
        target = "sink"
        volume = 3
    "#
    );

    assert_eq!(
        evaluate([("checking", 7), ("sink", 0)]),
        Ok([("checking", 4), ("sink", 3)])
    );
}

#[test]
fn facade_reexports_runtime_evaluator() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "sink".to_owned(),
        NodeConfig {
            capacity: None,
            target: None,
        },
    );
    let config = Config {
        default_bucket_target: None,
        default_flow_source: None,
        default_flow_target: None,
        nodes,
        flow: Vec::new(),
    };

    assert_eq!(
        clepsydra(&config).and_then(|evaluator| evaluator.eval([("sink", 4)])),
        Ok([("sink", 4)])
    );
}
