use crate::arena::{Arena, ArenaId};
use crate::config::Config;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

pub type NodeId<'brand> = ArenaId<'brand, Node<'brand>>;
pub type DAG<'brand> = Arena<'brand, Node<'brand>>;

#[derive(Debug)]
pub enum Node<'brand> {
    Sink,
    Bucket {
        capacity: u64,
        target: NodeId<'brand>,
    },
}

#[macro_export]
macro_rules! new_graph {
    () => {{
        let graph: $crate::topology::DAG<'_> = $crate::new_arena!();
        graph
    }};
}

#[derive(Debug)]
pub struct Flow<'brand> {
    pub source: NodeId<'brand>,
    pub target: NodeId<'brand>,
    pub volume: u64,
}

#[derive(Debug)]
pub struct Topology<'brand> {
    graph: DAG<'brand>,
    flows: Vec<Flow<'brand>>,

    name_to_id: HashMap<String, NodeId<'brand>>,
    id_to_name: HashMap<NodeId<'brand>, String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TopologyError {
    MissingCapacity { node: String },
    MissingTarget { node: String },
    UnresolvedTargets { nodes: Vec<String> },
    MissingFlowSource { flow: usize },
    MissingFlowTarget { flow: usize },
    UnknownFlowSource { flow: usize, node: String },
    UnknownFlowTarget { flow: usize, node: String },
}

impl Display for TopologyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCapacity { node } => {
                write!(formatter, "Node '{node}' has a target but no capacity")
            }
            Self::MissingTarget { node } => write!(
                formatter,
                "Node '{node}' has capacity but no target (and no default_target exists)"
            ),
            Self::UnresolvedTargets { nodes } => write!(
                formatter,
                "Cycle detected or missing target node involving: {nodes:?}"
            ),
            Self::MissingFlowSource { .. } => write!(formatter, "Missing Flow source"),
            Self::MissingFlowTarget { .. } => write!(formatter, "Missing Flow target"),
            Self::UnknownFlowSource { node, .. } => {
                write!(formatter, "Flow source '{node}' not found")
            }
            Self::UnknownFlowTarget { node, .. } => {
                write!(formatter, "Flow target '{node}' not found")
            }
        }
    }
}

impl std::error::Error for TopologyError {}

#[macro_export]
macro_rules! new_topology {
    ($config:expr) => {{
        let graph = $crate::new_graph!();
        $crate::topology::Topology::__new($config, graph)
    }};
}

impl<'brand> Topology<'brand> {
    #[must_use]
    pub const fn graph(&self) -> &DAG<'brand> {
        &self.graph
    }

    #[must_use]
    pub fn flows(&self) -> &[Flow<'brand>] {
        &self.flows
    }

    #[must_use]
    pub fn node_id(&self, name: &str) -> Option<NodeId<'brand>> {
        self.name_to_id.get(name).copied()
    }

    #[must_use]
    pub fn node_name(&self, id: NodeId<'brand>) -> Option<&str> {
        self.id_to_name.get(&id).map(String::as_str)
    }

    pub(crate) const fn name_to_id_map(&self) -> &HashMap<String, NodeId<'brand>> {
        &self.name_to_id
    }

    pub(crate) const fn id_to_name_map(&self) -> &HashMap<NodeId<'brand>, String> {
        &self.id_to_name
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.graph.len()
    }

    /// # Errors
    ///
    /// Returns an error when a node or flow is incomplete, references an unknown
    /// node, or the node targets cannot be resolved.
    pub fn __new(config: &Config, mut graph: DAG<'brand>) -> Result<Self, TopologyError> {
        let mut name_to_id = HashMap::new();
        let mut id_to_name = HashMap::new();

        let mut unprocessed: Vec<(&String, &crate::config::NodeConfig)> =
            config.nodes.iter().collect();

        while !unprocessed.is_empty() {
            let mut resolved_any = false;
            let mut next_unprocessed = Vec::new();

            for (name, config_node) in unprocessed {
                match (
                    &config_node.target,
                    &config_node.capacity,
                    &config.default_bucket_target,
                ) {
                    (Some(_), None, _) => {
                        return Err(TopologyError::MissingCapacity { node: name.clone() });
                    }
                    (None, Some(_), None) => {
                        return Err(TopologyError::MissingTarget { node: name.clone() });
                    }
                    (None, None, _) => {
                        let id = graph.push(Node::Sink);
                        name_to_id.insert(name.clone(), id);
                        id_to_name.insert(id, name.clone());
                        resolved_any = true;
                    }
                    (Some(t), Some(cap), _) | (None, Some(cap), Some(t)) => match name_to_id.get(t)
                    {
                        Some(&target_id) => {
                            let id = graph.push(Node::Bucket {
                                capacity: *cap,
                                target: target_id,
                            });
                            name_to_id.insert(name.clone(), id);
                            id_to_name.insert(id, name.clone());
                            resolved_any = true;
                        }
                        None => {
                            next_unprocessed.push((name, config_node));
                        }
                    },
                }
            }

            if !resolved_any {
                let cycle_names = next_unprocessed
                    .iter()
                    .map(|(name, _)| (*name).clone())
                    .collect();
                return Err(TopologyError::UnresolvedTargets { nodes: cycle_names });
            }

            unprocessed = next_unprocessed;
        }

        let mut flows = Vec::new();
        for (flow, dir) in config.flow.iter().enumerate() {
            let src_str = dir
                .source
                .as_ref()
                .or(config.default_flow_source.as_ref())
                .ok_or(TopologyError::MissingFlowSource { flow })?;

            let tgt_str = dir
                .target
                .as_ref()
                .or(config.default_flow_target.as_ref())
                .ok_or(TopologyError::MissingFlowTarget { flow })?;

            let source =
                *name_to_id
                    .get(src_str)
                    .ok_or_else(|| TopologyError::UnknownFlowSource {
                        flow,
                        node: src_str.clone(),
                    })?;

            let target =
                *name_to_id
                    .get(tgt_str)
                    .ok_or_else(|| TopologyError::UnknownFlowTarget {
                        flow,
                        node: tgt_str.clone(),
                    })?;

            flows.push(Flow {
                source,
                target,
                volume: dir.volume,
            });
        }

        Ok(Self {
            graph,
            flows,
            name_to_id,
            id_to_name,
        })
    }
}
