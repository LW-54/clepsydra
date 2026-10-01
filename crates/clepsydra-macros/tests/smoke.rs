use clepsydra_macros::{clepsydra, clepsydra_closure};

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
fn closure_expands_and_evaluates() {
    let evaluate = clepsydra_closure!(
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
    assert_eq!(evaluate(state), [("checking", 4), ("sink", 3)]);
}

#[test]
fn named_macro_exposes_metadata_and_evaluation() {
    assert_eq!(Ledger::NODE_COUNT, 2);
    assert_eq!(Ledger::NAMES, ["sink", "checking"]);
    assert_eq!(
        Ledger::evaluate([("checking", 7), ("sink", 0)]),
        [("checking", 4), ("sink", 3)]
    );
}
