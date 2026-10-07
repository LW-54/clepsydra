use crate::arena::Arena;
use crate::ast::{AST, AstBuilder, ExprId};
use crate::topology::{
    Behavior::{Bucket, Sink},
    Node, NodeId, Topology,
};
use std::collections::HashMap;

pub struct SymbolicState<'brand> {
    ast: AST<'brand, NodeId<'brand>>,
    node_exprs: HashMap<NodeId<'brand>, ExprId<'brand, NodeId<'brand>>>,
}

impl<'brand> SymbolicState<'brand> {
    #[must_use]
    pub const fn ast(&self) -> &AST<'brand, NodeId<'brand>> {
        &self.ast
    }

    #[must_use]
    pub const fn node_exprs(&self) -> &HashMap<NodeId<'brand>, ExprId<'brand, NodeId<'brand>>> {
        &self.node_exprs
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.node_exprs.len()
    }

    #[must_use]
    pub fn new(topology: &Topology<'brand>) -> Self {
        let mut ast: AST<'brand, NodeId<'brand>> = Arena::new(());

        let mut node_exprs = HashMap::new();
        macro_rules! get_expr {
            ($node_id:expr) => {{
                let node_id = $node_id;
                if let Some(&expr) = node_exprs.get(&node_id) {
                    expr
                } else {
                    let expr = ast.var(node_id);
                    node_exprs.insert(node_id, expr);
                    expr
                }
            }};
        }

        for (node_id, _) in topology.graph().iter() {
            node_exprs.insert(node_id, ast.var(node_id));
        }

        for (node_id, node) in topology.graph().iter().rev() {
            if let Node {
                behavior:
                    Bucket {
                        capacity: c,
                        target: t,
                        ..
                    },
                ..
            } = node
            {
                let expr = get_expr!(node_id);
                let c_expr = ast.const_val(*c);
                let overflow = ast.sat_sub(expr, c_expr);
                node_exprs.insert(node_id, ast.sat_sub(expr, overflow));
                let target_expr = get_expr!(*t);
                node_exprs.insert(*t, ast.add(target_expr, overflow));
            }
        }

        macro_rules! spillover {
            ($node_id:expr, $expr:expr) => {{
                let mut node_id = $node_id;
                let mut expr = $expr;
                loop {
                    match topology.graph().get(node_id) {
                        Node {
                            behavior:
                                Bucket {
                                    capacity: c,
                                    target: t,
                                    ..
                                },
                            ..
                        } => {
                            let c_expr = ast.const_val(*c);
                            let current_expr = get_expr!(node_id);
                            let total_expr = ast.add(current_expr, expr);
                            let overflow = ast.sat_sub(total_expr, c_expr);
                            node_exprs.insert(node_id, ast.sat_sub(total_expr, overflow));

                            node_id = *t;
                            expr = overflow;
                        }
                        Node { behavior: Sink, .. } => {
                            let node_expr = get_expr!(node_id);
                            node_exprs.insert(node_id, ast.add(node_expr, expr));
                            break;
                        }
                    }
                }
            }};
        }

        for (source, target, volume) in topology
            .flows()
            .iter()
            .map(|flow| (flow.source, flow.target, flow.volume))
        {
            let source_expr = get_expr!(source);
            let volume_expr = ast.const_val(volume);
            let remaining_expr = ast.sat_sub(source_expr, volume_expr);
            let transfer_expr = ast.sat_sub(source_expr, remaining_expr);
            node_exprs.insert(source, remaining_expr);
            spillover!(target, transfer_expr);
        }

        Self { ast, node_exprs }
    }
}
