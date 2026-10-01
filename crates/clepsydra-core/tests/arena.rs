use clepsydra_core::new_arena;

#[test]
fn arena_ids_survive_growth_and_iteration() {
    let mut arena = new_arena!();
    let first = arena.push("first");
    let second = arena.push("second");

    for _ in 0..32 {
        arena.push("extra");
    }

    assert_eq!(arena.get(first), &"first");
    assert_eq!(arena.get(second), &"second");
    assert_eq!(arena.len(), 34);
    assert!(!arena.is_empty());

    let values: Vec<_> = arena.iter().map(|(_, value)| *value).collect();
    assert_eq!(&values[..2], &["first", "second"]);
}
