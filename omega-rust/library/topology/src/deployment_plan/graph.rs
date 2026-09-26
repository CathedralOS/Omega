//! Bounded deterministic graph normalization and traversals.
//!
//! `NormalizedGraph` is the only graph shape the predicates run over. Its
//! construction is total and terminating: instance and edge counts are
//! bounded by the codec limits, the roster sorts by name (so vertex index
//! order equals canonical name order), and adjacency is deduplicated while
//! every binding's evidence is retained. Traversals visit each vertex and
//! edge at most once under an explicit visited set — cycles and
//! self-connections cannot cause non-termination.
//!
//! There are no implicit edges: equal contracts, matching names, or
//! co-location never connect instances. Only a recorded `Binding` between a
//! demanded import and a declared export creates a directed edge, in the
//! import-to-export (call) direction.

use crate::deployment_plan::{Binding, EndpointDirection, EndpointKey, InstanceName, PlanInstance};
use std::collections::BTreeSet;
use std::fmt;

/// Failures of graph normalization. Each names the offending record; these
/// are invalid compositions, not policy verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Two roster entries share one `InstanceKey`.
    DuplicateInstance { name: InstanceName },
    /// An instance declares two endpoints with the same `(slot, direction)`.
    DuplicateEndpoint { instance: InstanceName, slot: u32 },
    /// A binding names an instance index outside the roster.
    UnknownInstance { index: u32 },
    /// A binding names a slot the instance does not declare.
    UnknownEndpoint { key: EndpointKey },
    /// A binding's import/export keys resolve to the wrong direction.
    WrongDirection {
        key: EndpointKey,
        expected: EndpointDirection,
    },
    /// The same demanded import is bound more than once, or the same binding
    /// row appears twice.
    DuplicateBinding { import: EndpointKey },
    /// A demanded import has no binding.
    UnboundImport { key: EndpointKey },
    /// An exact identity field was left as the reserved zero identity.
    NullIdentity { field: &'static str },
    /// A bound in the codec contract was exceeded; exhaustion is an
    /// unsuccessful verification, not a satisfied or violated predicate.
    LimitExceeded { what: &'static str },
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInstance { name } => {
                write!(formatter, "duplicate instance name `{name}`")
            }
            Self::DuplicateEndpoint { instance, slot } => {
                write!(
                    formatter,
                    "instance `{instance}` declares two endpoints at slot {slot}"
                )
            }
            Self::UnknownInstance { index } => {
                write!(
                    formatter,
                    "binding names instance index {index}, outside the roster"
                )
            }
            Self::UnknownEndpoint { key } => {
                write!(
                    formatter,
                    "endpoint ({}, {}) is not declared by its instance",
                    key.instance, key.slot
                )
            }
            Self::WrongDirection { key, expected } => {
                write!(
                    formatter,
                    "endpoint ({}, {}) is not an {expected:?} endpoint",
                    key.instance, key.slot
                )
            }
            Self::DuplicateBinding { import } => {
                write!(
                    formatter,
                    "demanded import ({}, {}) is bound more than once",
                    import.instance, import.slot
                )
            }
            Self::UnboundImport { key } => {
                write!(
                    formatter,
                    "demanded import ({}, {}) has no binding",
                    key.instance, key.slot
                )
            }
            Self::NullIdentity { field } => {
                write!(formatter, "{field} uses the reserved zero identity")
            }
            Self::LimitExceeded { what } => {
                write!(formatter, "bounded limit exceeded: {what}")
            }
        }
    }
}

impl std::error::Error for GraphError {}

/// The checked, canonical instance graph.
#[derive(Debug, Clone)]
pub struct NormalizedGraph {
    /// Instances sorted by name; vertex index order is canonical name order.
    instances: Vec<PlanInstance>,
    /// Deduplicated, sorted successor indices per vertex.
    adjacency: Vec<Vec<u32>>,
    /// Every binding in canonical order (evidence is never dropped).
    bindings: Vec<Binding>,
}

impl NormalizedGraph {
    /// Normalize a roster and binding set into the checked graph. Input order
    /// does not matter — output is canonical. This is also where complete
    /// binding coverage is established: every demanded import binds exactly
    /// once to a declared export.
    pub fn new(
        mut instances: Vec<PlanInstance>,
        mut bindings: Vec<Binding>,
    ) -> Result<Self, GraphError> {
        if instances.len() > crate::deployment_plan::codec::MAX_INSTANCES {
            return Err(GraphError::LimitExceeded { what: "instances" });
        }
        if bindings.len() > crate::deployment_plan::codec::MAX_BINDINGS {
            return Err(GraphError::LimitExceeded { what: "bindings" });
        }

        instances.sort_by(|left, right| left.name.cmp(&right.name));
        for pair in instances.windows(2) {
            if pair[0].name == pair[1].name {
                return Err(GraphError::DuplicateInstance {
                    name: pair[0].name.clone(),
                });
            }
        }
        for instance in &mut instances {
            if instance.component.subject == [0u8; 32] {
                return Err(GraphError::NullIdentity {
                    field: "component subject",
                });
            }
            if instance.endpoints.len() > crate::deployment_plan::codec::MAX_ENDPOINTS_PER_INSTANCE
            {
                return Err(GraphError::LimitExceeded {
                    what: "endpoints per instance",
                });
            }
            instance
                .endpoints
                .sort_by_key(|endpoint| (endpoint.slot, endpoint.direction));
            for pair in instance.endpoints.windows(2) {
                if pair[0].slot == pair[1].slot && pair[0].direction == pair[1].direction {
                    return Err(GraphError::DuplicateEndpoint {
                        instance: instance.name.clone(),
                        slot: pair[0].slot,
                    });
                }
            }
        }

        // Resolve each binding's keys against the declared inventories.
        bindings.sort_by_key(|binding| (binding.import, binding.export));
        let mut bound_imports: BTreeSet<EndpointKey> = BTreeSet::new();
        let mut adjacency_sets: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); instances.len()];
        for binding in &bindings {
            for (key, expected) in [
                (binding.import, EndpointDirection::Import),
                (binding.export, EndpointDirection::Export),
            ] {
                let Some(instance) = instances.get(key.instance as usize) else {
                    return Err(GraphError::UnknownInstance {
                        index: key.instance,
                    });
                };
                // Slots key endpoints per direction: an interface may carry
                // an import and an export on one slot as distinct endpoints.
                match instance
                    .endpoints
                    .iter()
                    .find(|endpoint| endpoint.slot == key.slot && endpoint.direction == expected)
                {
                    Some(_) => {}
                    None if instance
                        .endpoints
                        .iter()
                        .any(|endpoint| endpoint.slot == key.slot) =>
                    {
                        return Err(GraphError::WrongDirection { key, expected });
                    }
                    None => return Err(GraphError::UnknownEndpoint { key }),
                }
            }
            if binding.transport == [0u8; 32] {
                return Err(GraphError::NullIdentity {
                    field: "binding transport",
                });
            }
            if !bound_imports.insert(binding.import) {
                return Err(GraphError::DuplicateBinding {
                    import: binding.import,
                });
            }
            adjacency_sets[binding.import.instance as usize].insert(binding.export.instance);
        }
        // Complete bindings: every demanded import has exactly one binding.
        for (index, instance) in instances.iter().enumerate() {
            for endpoint in &instance.endpoints {
                if endpoint.direction != EndpointDirection::Import {
                    continue;
                }
                let key = EndpointKey {
                    instance: index as u32,
                    slot: endpoint.slot,
                };
                if !bound_imports.contains(&key) {
                    return Err(GraphError::UnboundImport { key });
                }
            }
        }

        let adjacency = adjacency_sets
            .into_iter()
            .map(|set| set.into_iter().collect())
            .collect();
        Ok(Self {
            instances,
            adjacency,
            bindings,
        })
    }

    pub fn instances(&self) -> &[PlanInstance] {
        &self.instances
    }

    /// Canonical vertex index of an instance name.
    pub fn index_of(&self, name: &InstanceName) -> Option<u32> {
        self.instances
            .binary_search_by(|instance| instance.name.cmp(name))
            .ok()
            .map(|index| index as u32)
    }

    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Deduplicated successors of a vertex, in canonical order.
    pub fn successors(&self, vertex: u32) -> &[u32] {
        &self.adjacency[vertex as usize]
    }

    /// The vertices reachable from `sources`, sorted, with `excluded`
    /// vertices removed from the graph. Worklist traversal: each vertex is
    /// visited at most once.
    pub fn reachable_from(&self, sources: &BTreeSet<u32>, excluded: &BTreeSet<u32>) -> Vec<u32> {
        let mut visited = sources.clone();
        let mut worklist: Vec<u32> = sources.iter().copied().collect();
        while let Some(vertex) = worklist.pop() {
            for &successor in self.successors(vertex) {
                if excluded.contains(&successor) || !visited.insert(successor) {
                    continue;
                }
                worklist.push(successor);
            }
        }
        visited.retain(|vertex| !excluded.contains(vertex));
        visited.into_iter().collect()
    }

    /// The canonical shortest path from any vertex in `sources` to any vertex
    /// in `targets`, optionally avoiding `excluded`. Multi-source BFS with
    /// successors expanded in canonical (name) order yields the shortest
    /// witness, tie-broken lexicographically by vertex index — the reference
    /// checker's required witness choice. Returns `None` when no path exists.
    pub fn shortest_path(
        &self,
        sources: &BTreeSet<u32>,
        targets: &BTreeSet<u32>,
        excluded: &BTreeSet<u32>,
    ) -> Option<Vec<u32>> {
        let mut parent: Vec<Option<u32>> = vec![None; self.instances.len()];
        let mut visited = excluded.clone();
        let mut queue: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
        for &source in sources {
            if visited.insert(source) {
                queue.push_back(source);
            }
        }
        while let Some(vertex) = queue.pop_front() {
            if targets.contains(&vertex) {
                let mut path = vec![vertex];
                let mut cursor = vertex;
                while let Some(previous) = parent[cursor as usize] {
                    path.push(previous);
                    cursor = previous;
                }
                path.reverse();
                return Some(path);
            }
            for &successor in self.successors(vertex) {
                if visited.insert(successor) {
                    parent[successor as usize] = Some(vertex);
                    queue.push_back(successor);
                }
            }
        }
        None
    }

    /// Whether `path` is a genuine traversal: first vertex in `sources`, last
    /// in `targets`, every consecutive pair an edge, and — when `excluded` is
    /// given — no vertex passing through it.
    pub fn path_is_valid(
        &self,
        path: &[u32],
        sources: &BTreeSet<u32>,
        targets: &BTreeSet<u32>,
        excluded: &BTreeSet<u32>,
    ) -> bool {
        let (Some(&first), Some(&last)) = (path.first(), path.last()) else {
            return false;
        };
        if !sources.contains(&first) || !targets.contains(&last) {
            return false;
        }
        if path.len() > self.instances.len() {
            return false;
        }
        path.windows(2)
            .all(|pair| self.successors(pair[0]).contains(&pair[1]))
            && path.iter().all(|vertex| !excluded.contains(vertex))
    }
}
