use crate::ast::{AST, Expr, ExprId};
use crate::config::Config;
use crate::errors::EvalError;
use crate::new_graph;
use crate::symbolic::SymbolicState;
use crate::topology::{NodeId, Topology};
use std::collections::HashMap;
use std::hash::BuildHasher;

pub type NamedEvaluator = Box<
    dyn for<'state> Fn(&'state [(&'state str, u64)]) -> Result<Vec<(&'state str, u64)>, EvalError>
        + 'static,
>;
pub type MapEvaluator =
    Box<dyn Fn(HashMap<String, u64>) -> Result<HashMap<String, u64>, EvalError> + 'static>;

pub struct RuntimeEvaluator<'brand> {
    topology: Topology<'brand>,
    symbolic: SymbolicState<'brand>,
    node_count: usize,
    names: Vec<String>,
}

impl<'brand> RuntimeEvaluator<'brand> {
    #[must_use]
    pub fn new(topology: Topology<'brand>) -> Self {
        let symbolic = SymbolicState::new(&topology);
        let names = topology
            .graph()
            .iter()
            .map(|(node, _)| topology.node_name(node).to_owned())
            .collect();
        let node_count = topology.node_count();
        Self {
            topology,
            symbolic,
            node_count,
            names,
        }
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Evaluates values in topology order.
    ///
    /// # Errors
    ///
    /// Returns an error when the array length does not match the topology.
    pub fn ordered_eval<const N: usize>(&self, state: [u64; N]) -> Result<[u64; N], EvalError> {
        if N != self.node_count {
            return Err(EvalError::InvalidInputCount {
                expected: self.node_count,
                actual: N,
            });
        }
        let values = self.evaluate_ordered_values(&self.ordered_input_values(state))?;
        values
            .try_into()
            .map_err(|values: Vec<u64>| EvalError::InvalidInputCount {
                expected: N,
                actual: values.len(),
            })
    }

    /// Evaluates named values while preserving caller order.
    ///
    /// # Errors
    ///
    /// Returns an error when names are invalid or the array length is wrong.
    pub fn eval<'state, const N: usize>(
        &self,
        state: [(&'state str, u64); N],
    ) -> Result<[(&'state str, u64); N], EvalError> {
        let output = self.vec_eval(&state)?;
        output.try_into().map_err(
            |output: Vec<(&'state str, u64)>| EvalError::InvalidInputCount {
                expected: N,
                actual: output.len(),
            },
        )
    }

    /// Evaluates named values while preserving caller order.
    ///
    /// # Errors
    ///
    /// Returns an error when names are invalid, duplicated, missing, or when
    /// the input count does not match the topology.
    pub fn vec_eval<'state>(
        &self,
        state: &[(&'state str, u64)],
    ) -> Result<Vec<(&'state str, u64)>, EvalError> {
        if state.len() != self.node_count {
            return Err(EvalError::InvalidInputCount {
                expected: self.node_count,
                actual: state.len(),
            });
        }

        let mut ordered = vec![0_u64; self.node_count];
        let mut seen = vec![false; self.node_count];
        for (name, value) in state.iter().copied() {
            let Some(node) = self.topology.node_id(name) else {
                return Err(EvalError::UnknownInput {
                    name: name.to_owned(),
                });
            };
            let Some(slot) = self
                .topology
                .graph()
                .iter()
                .position(|(candidate, _)| candidate == node)
            else {
                return Err(EvalError::UnknownInput {
                    name: name.to_owned(),
                });
            };
            if seen.get(slot).copied().unwrap_or(false) {
                return Err(EvalError::DuplicateInput {
                    name: name.to_owned(),
                });
            }
            let Some(seen_slot) = seen.get_mut(slot) else {
                return Err(EvalError::InvalidInputCount {
                    expected: self.node_count,
                    actual: state.len(),
                });
            };
            *seen_slot = true;
            let Some(output) = ordered.get_mut(slot) else {
                return Err(EvalError::InvalidInputCount {
                    expected: self.node_count,
                    actual: state.len(),
                });
            };
            *output = value;
        }

        let result = self.evaluate_ordered_values(&self.ordered_input_values_slice(&ordered))?;
        state
            .iter()
            .copied()
            .map(|(name, _)| {
                let Some(slot) = self.names.iter().position(|candidate| candidate == name) else {
                    return Err(EvalError::UnknownInput {
                        name: name.to_owned(),
                    });
                };
                let Some(value) = result.get(slot).copied() else {
                    return Err(EvalError::MissingInput {
                        name: name.to_owned(),
                    });
                };
                Ok((name, value))
            })
            .collect()
    }

    /// Evaluates owned map input and returns owned map output.
    ///
    /// # Errors
    ///
    /// Returns an error when names are unknown or missing. A `HashMap` cannot
    /// contain duplicate keys.
    pub fn map_eval<S: BuildHasher>(
        &self,
        state: HashMap<String, u64, S>,
    ) -> Result<HashMap<String, u64>, EvalError> {
        if state.len() != self.node_count {
            return Err(EvalError::InvalidInputCount {
                expected: self.node_count,
                actual: state.len(),
            });
        }
        let mut values = vec![0_u64; self.node_count];
        let mut seen = vec![false; self.node_count];
        for (name, value) in state {
            let Some(slot) = self.names.iter().position(|candidate| candidate == &name) else {
                return Err(EvalError::UnknownInput { name });
            };
            if seen.get(slot).copied().unwrap_or(false) {
                return Err(EvalError::DuplicateInput { name });
            }
            let Some(seen_slot) = seen.get_mut(slot) else {
                return Err(EvalError::InvalidInputCount {
                    expected: self.node_count,
                    actual: self.node_count,
                });
            };
            *seen_slot = true;
            let Some(value_slot) = values.get_mut(slot) else {
                return Err(EvalError::InvalidInputCount {
                    expected: self.node_count,
                    actual: self.node_count,
                });
            };
            *value_slot = value;
        }
        let result = self.evaluate_ordered_values(&self.ordered_input_values_slice(&values))?;
        Ok(self.names.iter().cloned().zip(result).collect())
    }

    fn ordered_input_values<const N: usize>(
        &self,
        state: [u64; N],
    ) -> HashMap<NodeId<'brand>, u64> {
        self.topology
            .graph()
            .iter()
            .zip(state)
            .map(|((node, _), value)| (node, value))
            .collect()
    }

    fn ordered_input_values_slice(&self, state: &[u64]) -> HashMap<NodeId<'brand>, u64> {
        self.topology
            .graph()
            .iter()
            .zip(state.iter().copied())
            .map(|((node, _), value)| (node, value))
            .collect()
    }

    fn evaluate_ordered_values(
        &self,
        node_values: &HashMap<NodeId<'brand>, u64>,
    ) -> Result<Vec<u64>, EvalError> {
        let mut expressions = HashMap::new();
        self.topology
            .graph()
            .iter()
            .map(|(node, _)| {
                let expression =
                    self.symbolic
                        .node_exprs()
                        .get(&node)
                        .copied()
                        .ok_or_else(|| EvalError::MissingInput {
                            name: self.node_name(node),
                        })?;
                evaluate_expr(
                    self.symbolic.ast(),
                    expression,
                    node_values,
                    &mut expressions,
                )
                .ok_or_else(|| EvalError::MissingInput {
                    name: self.node_name(node),
                })
            })
            .collect()
    }

    fn node_name(&self, node: NodeId<'brand>) -> String {
        self.topology.node_name(node).to_owned()
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

/// # Errors
/// Returns a topology error when configuration is invalid.
pub fn clepsydra(config: &Config) -> Result<RuntimeEvaluator<'static>, EvalError> {
    let graph = new_graph!();
    Ok(RuntimeEvaluator::new(Topology::__new(config, graph)?))
}

/// # Errors
/// Returns a topology error when configuration is invalid.
pub fn clepsydra_eval(config: &Config) -> Result<NamedEvaluator, EvalError> {
    clepsydra_vec_eval(config)
}

/// # Errors
/// Returns a topology error when configuration is invalid.
pub fn clepsydra_vec_eval(config: &Config) -> Result<NamedEvaluator, EvalError> {
    let evaluator = clepsydra(config)?;
    Ok(Box::new(move |state| evaluator.vec_eval(state)))
}

/// # Errors
/// Returns a topology error when configuration is invalid.
pub fn clepsydra_map_eval(config: &Config) -> Result<MapEvaluator, EvalError> {
    let evaluator = clepsydra(config)?;
    Ok(Box::new(move |state| evaluator.map_eval(state)))
}
