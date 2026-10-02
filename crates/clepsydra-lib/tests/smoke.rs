use clepsydra_lib::clepsydra_closure;

#[test]
fn facade_reexports_closure_macro() {
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

    assert_eq!(
        evaluate([("checking", 7), ("sink", 0)]),
        [("checking", 4), ("sink", 3)]
    );
}
