use clepsydra_core::errors::EvalError;
use clepsydra_macros::{clepsydra, clepsydra_eval, clepsydra_map_eval, clepsydra_vec_eval};
use std::collections::HashMap;

clepsydra!(
    Ledger,
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

#[test]
fn eval_macro_expands_and_preserves_input_order() {
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

    let state = [("checking", 7), ("sink", 0)];
    assert_eq!(evaluate(state), Ok([("checking", 4), ("sink", 3)]));
}

#[test]
fn generated_evaluators_accept_locally_borrowed_names() {
    let checking = String::from("checking");
    let sink = String::from("sink");
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
        evaluate([(&checking, 7), (&sink, 0)]),
        Ok([(checking.as_str(), 4), (sink.as_str(), 3)])
    );
    assert_eq!(
        Ledger::eval([(&sink, 0), (&checking, 7)]),
        Ok([(sink.as_str(), 3), (checking.as_str(), 4)])
    );
}

#[test]
fn named_macro_exposes_metadata_and_evaluation() {
    assert_eq!(Ledger::NODE_COUNT, 2);
    assert_eq!(Ledger::NAMES, ["sink", "checking"]);
    assert_eq!(Ledger::ordered_eval([0, 7]), [3, 4]);
}

#[test]
fn eval_accepts_json_and_yaml() {
    let json = clepsydra_eval!(
        r#"{
        "nodes": {"sink": {"capacity": null, "target": null}}
    }"#
    );
    let yaml = clepsydra_eval!(
        r#"
        nodes:
          sink:
            capacity: null
            target: null
    "#
    );

    assert_eq!(json([("sink", 4)]), Ok([("sink", 4)]));
    assert_eq!(yaml([("sink", 4)]), Ok([("sink", 4)]));
}

#[test]
fn named_evaluation_rejects_invalid_names() {
    assert!(matches!(
        Ledger::eval([("sink", 0), ("sink", 7)]),
        Err(EvalError::DuplicateInput { name }) if name == "sink"
    ));
    assert!(matches!(
        Ledger::eval([("unknown", 0), ("checking", 7)]),
        Err(EvalError::UnknownInput { name }) if name == "unknown"
    ));
}

#[test]
fn dictionary_evaluation_uses_the_same_generated_computation() {
    let mut state = HashMap::new();
    state.insert("checking".to_owned(), 7);
    state.insert("sink".to_owned(), 0);

    let mut expected = HashMap::new();
    expected.insert("checking".to_owned(), 4);
    expected.insert("sink".to_owned(), 3);
    assert_eq!(Ledger::map_eval(state), Ok(expected));
}

#[test]
fn dictionary_macro_returns_a_dictionary_evaluator() {
    let evaluate = clepsydra_map_eval!(r#"[nodes.sink]"#);
    let state = HashMap::from([(String::from("sink"), 4)]);

    assert_eq!(
        evaluate(state),
        Ok(HashMap::from([(String::from("sink"), 4)]))
    );
}

#[test]
fn vec_macro_returns_a_vec_evaluator() {
    let evaluate = clepsydra_vec_eval!(r#"[nodes.sink]"#);
    assert_eq!(evaluate(&[("sink", 4)]), Ok(vec![("sink", 4)]));
}
