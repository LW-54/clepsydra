use clepsydra_core::ast::{AST, Expr, ExprId};
use clepsydra_core::config::{Config, FlowConfig, NodeConfig};
use clepsydra_core::new_graph;
use clepsydra_core::symbolic::SymbolicState;
use clepsydra_core::topology::{Node, NodeId, Topology};
use proptest::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug)]
struct ValidCase {
    config: Config,
    initial: Vec<u64>,
}

fn arb_valid_case() -> impl Strategy<Value = ValidCase> {
    (
        prop::collection::vec((0_u64..=1_000, any::<usize>(), 0_u64..=1_000), 1..=8),
        prop::collection::vec((any::<usize>(), any::<usize>(), 0_u64..=1_000), 0..=8),
    )
        .prop_map(|(nodes, flows)| {
            let mut config_nodes = BTreeMap::new();
            let mut initial = Vec::with_capacity(nodes.len());

            for (index, (capacity, target, value)) in nodes.into_iter().enumerate() {
                let name = format!("n{index}");
                if index == 0 {
                    config_nodes.insert(
                        name,
                        NodeConfig {
                            capacity: None,
                            target: None,
                        },
                    );
                } else {
                    config_nodes.insert(
                        name,
                        NodeConfig {
                            capacity: Some(capacity),
                            target: Some(format!("n{}", target.rem_euclid(index))),
                        },
                    );
                }
                initial.push(value);
            }

            let node_count = config_nodes.len();
            let flow = flows
                .into_iter()
                .map(|(source, target, volume)| FlowConfig {
                    source: Some(format!("n{}", source.rem_euclid(node_count))),
                    target: Some(format!("n{}", target.rem_euclid(node_count))),
                    volume,
                })
                .collect();

            ValidCase {
                config: Config {
                    default_bucket_target: None,
                    default_flow_source: None,
                    default_flow_target: None,
                    nodes: config_nodes,
                    flow,
                },
                initial,
            }
        })
}

fn evaluate<'brand>(
    symbolic: &SymbolicState<'brand>,
    initial: &[u64],
) -> Option<HashMap<NodeId<'brand>, u64>> {
    fn evaluate_expr<'brand>(
        ast: &AST<'brand, NodeId<'brand>>,
        id: ExprId<'brand, NodeId<'brand>>,
        node_values: &HashMap<NodeId<'brand>, u64>,
        expressions: &mut HashMap<ExprId<'brand, NodeId<'brand>>, u64>,
    ) -> Option<u64> {
        if let Some(value) = expressions.get(&id).copied() {
            return Some(value);
        }

        let value = match ast.get(id) {
            Expr::Var(node) => node_values.get(node).copied()?,
            Expr::Const(value) => *value,
            Expr::Add(lhs, rhs) => evaluate_expr(ast, *lhs, node_values, expressions)?
                .saturating_add(evaluate_expr(ast, *rhs, node_values, expressions)?),
            Expr::SatSub(lhs, rhs) => evaluate_expr(ast, *lhs, node_values, expressions)?
                .saturating_sub(evaluate_expr(ast, *rhs, node_values, expressions)?),
        };
        expressions.insert(id, value);
        Some(value)
    }

    let mut names: Vec<_> = symbolic.node_exprs().keys().copied().collect();
    names.sort_by(|left, right| symbolic.node_name(*left).cmp(&symbolic.node_name(*right)));
    let node_values: HashMap<_, _> = names.into_iter().zip(initial.iter().copied()).collect();
    let mut expressions = HashMap::new();

    symbolic
        .node_exprs()
        .iter()
        .map(|(node, expr)| {
            evaluate_expr(symbolic.ast(), *expr, &node_values, &mut expressions)
                .map(|value| (*node, value))
        })
        .collect()
}

fn assert_symbolic_invariants<'brand>(
    symbolic: &SymbolicState<'brand>,
    topology: &Topology<'brand>,
) {
    let topology_nodes: HashSet<_> = topology.graph().iter().map(|(id, _)| id).collect();
    let ast_indices: HashMap<_, _> = symbolic
        .ast()
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (id, index))
        .collect();

    assert_eq!(symbolic.node_exprs().len(), topology_nodes.len());
    assert_eq!(
        symbolic
            .node_exprs()
            .keys()
            .copied()
            .collect::<HashSet<_>>(),
        topology_nodes
    );

    let mut pending: Vec<_> = symbolic.node_exprs().values().copied().collect();
    let mut reachable = HashSet::new();
    while let Some(expr_id) = pending.pop() {
        if !reachable.insert(expr_id) {
            continue;
        }

        let Some(&expr_index) = ast_indices.get(&expr_id) else {
            assert!(
                ast_indices.contains_key(&expr_id),
                "node expression root is missing from the AST"
            );
            continue;
        };
        match symbolic.ast().get(expr_id) {
            Expr::Var(node_id) => assert!(topology_nodes.contains(node_id)),
            Expr::Const(_) => {}
            Expr::Add(lhs, rhs) | Expr::SatSub(lhs, rhs) => {
                for child in [*lhs, *rhs] {
                    let Some(&child_index) = ast_indices.get(&child) else {
                        assert!(
                            ast_indices.contains_key(&child),
                            "AST child is missing from the AST"
                        );
                        continue;
                    };
                    assert!(child_index < expr_index);
                    pending.push(child);
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn valid_configurations_conserve_volume(case in arb_valid_case()) {
        let graph = new_graph!();
        let topology = Topology::__new(&case.config, graph);
        prop_assert!(topology.is_ok());
        let Ok(topology) = topology else { return Ok(()) };
        let symbolic = SymbolicState::new(&topology);
        assert_symbolic_invariants(&symbolic, &topology);
        let final_values = evaluate(&symbolic, &case.initial);
        prop_assert!(final_values.is_some());
        let Some(final_values) = final_values else { return Ok(()) };

        let initial_total: u64 = case.initial.iter().sum();
        let final_total: u64 = final_values.values().sum();
        prop_assert_eq!(final_total, initial_total);
    }

    #[test]
    fn valid_configurations_respect_bucket_capacity(case in arb_valid_case()) {
        let graph = new_graph!();
        let topology = Topology::__new(&case.config, graph);
        prop_assert!(topology.is_ok());
        let Ok(topology) = topology else { return Ok(()) };
        let symbolic = SymbolicState::new(&topology);
        assert_symbolic_invariants(&symbolic, &topology);
        let final_values = evaluate(&symbolic, &case.initial);
        prop_assert!(final_values.is_some());
        let Some(final_values) = final_values else { return Ok(()) };

        for (id, node) in topology.graph().iter() {
            if let Node::Bucket { capacity, .. } = node {
                let Some(value) = final_values.get(&id) else {
                    prop_assert!(false, "missing final value for generated node");
                    return Ok(());
                };
                prop_assert!(*value <= *capacity);
            }
        }
    }
}
