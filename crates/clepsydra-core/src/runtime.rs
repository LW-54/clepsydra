use crate::ast::{AST, Expr, ExprId};
use crate::config::Config;
use crate::new_graph;
use crate::symbolic::SymbolicState;
use crate::topology::{NodeId, Topology, TopologyError};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeError {
    DuplicateInput { name: String },
    UnknownInput { name: String },
    MissingInput { name: String },
    InvalidInputCount { expected: usize, actual: usize },
    Topology(TopologyError),
}

impl Display for RuntimeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateInput { name } => write!(formatter, "Duplicate input '{name}'"),
            Self::UnknownInput { name } => write!(formatter, "Unknown input '{name}'"),
            Self::MissingInput { name } => write!(formatter, "Missing input '{name}'"),
            Self::InvalidInputCount { expected, actual } => write!(
                formatter,
                "Expected {expected} inputs, but received {actual}",
            ),
            Self::Topology(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<TopologyError> for RuntimeError {
    fn from(error: TopologyError) -> Self {
        Self::Topology(error)
    }
}

pub struct RuntimeEvaluator<'brand> {
    symbolic: &'brand SymbolicState<'brand>,
}

impl<'brand> RuntimeEvaluator<'brand> {
    #[must_use]
    pub const fn new(symbolic: &'brand SymbolicState<'brand>) -> Self {
        Self { symbolic }
    }

    /// Evaluates named node values and returns results sorted by node name.
    ///
    /// # Errors
    ///
    /// Returns an error when an input is duplicated, unknown, or missing.
    pub fn evaluate(&self, state: &[(&str, u64)]) -> Result<Vec<(String, u64)>, RuntimeError> {
        let node_values = self.input_values(state)?;
        let mut expressions = HashMap::new();
        let mut nodes: Vec<_> = self.symbolic.node_exprs().keys().copied().collect();
        nodes.sort_by(|left, right| {
            self.symbolic
                .node_name(*left)
                .cmp(&self.symbolic.node_name(*right))
        });

        nodes
            .into_iter()
            .map(|node| {
                let expression = self.symbolic.node_exprs().get(&node).copied();
                let Some(expression) = expression else {
                    return Err(RuntimeError::MissingInput {
                        name: self.node_name(node),
                    });
                };
                evaluate_expr(
                    self.symbolic.ast(),
                    expression,
                    &node_values,
                    &mut expressions,
                )
                .map(|value| (self.node_name(node), value))
                .ok_or_else(|| RuntimeError::MissingInput {
                    name: self.node_name(node),
                })
            })
            .collect()
    }

    /// Evaluates values while preserving the caller's fixed-array ordering.
    ///
    /// # Errors
    ///
    /// Returns an error when the array does not contain exactly one value for
    /// every configured node, or when an input name is unknown or duplicated.
    pub fn evaluate_array<'state, const N: usize>(
        &self,
        state: [(&'state str, u64); N],
    ) -> Result<[(&'state str, u64); N], RuntimeError> {
        if N != self.symbolic.node_count() {
            return Err(RuntimeError::InvalidInputCount {
                expected: self.symbolic.node_count(),
                actual: N,
            });
        }

        let values = self.evaluate(&state)?;
        let mut result = state;
        for (name, value) in values {
            let Some((_, output)) = result
                .iter_mut()
                .find(|(entry_name, _)| *entry_name == name)
            else {
                return Err(RuntimeError::MissingInput { name });
            };
            *output = value;
        }
        Ok(result)
    }

    fn input_values(
        &self,
        state: &[(&str, u64)],
    ) -> Result<HashMap<NodeId<'brand>, u64>, RuntimeError> {
        if state.len() != self.symbolic.node_count() {
            return Err(RuntimeError::InvalidInputCount {
                expected: self.symbolic.node_count(),
                actual: state.len(),
            });
        }

        let mut values = HashMap::with_capacity(state.len());
        for (name, value) in state {
            let Some(node) = self.symbolic.node_id(name) else {
                return Err(RuntimeError::UnknownInput {
                    name: (*name).to_owned(),
                });
            };
            if values.insert(node, *value).is_some() {
                return Err(RuntimeError::DuplicateInput {
                    name: (*name).to_owned(),
                });
            }
        }

        for node in self.symbolic.node_exprs().keys() {
            if !values.contains_key(node) {
                return Err(RuntimeError::MissingInput {
                    name: self.node_name(*node),
                });
            }
        }
        Ok(values)
    }

    fn node_name(&self, node: NodeId<'brand>) -> String {
        self.symbolic
            .node_name(node)
            .map_or_else(|| "<unknown>".to_owned(), ToOwned::to_owned)
    }
}

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

/// Builds and evaluates a topology without exposing its arena lifetime.
///
/// # Errors
///
/// Returns a topology error when the configuration is invalid, or a runtime
/// error when the supplied state does not identify every configured node.
pub fn evaluate_config(
    config: &Config,
    state: &[(&str, u64)],
) -> Result<Vec<(String, u64)>, RuntimeError> {
    let graph = new_graph!();
    let topology = Topology::__new(config, graph)?;
    let symbolic = SymbolicState::new(&topology);
    RuntimeEvaluator::new(&symbolic).evaluate(state)
}

/// Builds and evaluates a topology while preserving fixed-array ordering.
///
/// # Errors
///
/// Returns a topology error when the configuration is invalid, or a runtime
/// error when the supplied array does not identify every configured node.
pub fn evaluate_config_array<'state, const N: usize>(
    config: &Config,
    state: [(&'state str, u64); N],
) -> Result<[(&'state str, u64); N], RuntimeError> {
    let graph = new_graph!();
    let topology = Topology::__new(config, graph)?;
    let symbolic = SymbolicState::new(&topology);
    RuntimeEvaluator::new(&symbolic).evaluate_array(state)
}
