//! One borrowed predecessor tree for a batch of package explanation paths.

use super::ResolvedPackageSourceClosure;
use super::{DependencyRequestPath, DependencyRequestPathStep};
use crate::declarations::PackageKey;

/// Local to a comparison over an immutable closure. Entries name graph-owned
/// edges; package keys and aliases are copied only into requested output paths.
pub(crate) struct DependencyRequestPaths<'closure> {
    closure: &'closure ResolvedPackageSourceClosure,
    root_position: usize,
    predecessors: Vec<Option<(usize, usize)>>,
    #[cfg(test)]
    pub(super) traversed_edges: usize,
}

impl<'closure> DependencyRequestPaths<'closure> {
    pub(super) fn new(
        closure: &'closure ResolvedPackageSourceClosure,
        stop_at: Option<&PackageKey>,
    ) -> Option<Self> {
        let graph = closure.graph();
        let root_position = graph.package_position(graph.root())?;
        let mut predecessors = vec![None; graph.packages().len()];
        let mut pending = Vec::with_capacity(graph.packages().len());
        pending.push(root_position);
        let mut next = 0;
        #[cfg(test)]
        let mut traversed_edges = 0;
        'traverse: while next < pending.len() {
            let requester_position = pending[next];
            next += 1;
            let requester = &graph.packages()[requester_position];
            for (edge_position, dependency) in requester.dependencies().iter().enumerate() {
                #[cfg(test)]
                {
                    traversed_edges += 1;
                }
                let target_position = graph.package_position(dependency.target())?;
                if target_position == root_position || predecessors[target_position].is_some() {
                    continue;
                }
                predecessors[target_position] = Some((requester_position, edge_position));
                pending.push(target_position);
                if stop_at == Some(dependency.target()) {
                    break 'traverse;
                }
            }
        }
        Some(Self {
            closure,
            root_position,
            predecessors,
            #[cfg(test)]
            traversed_edges,
        })
    }

    /// Materialize the same shortest path as a fresh breadth-first search.
    /// Authored edge order decides ties, independent of output query order.
    pub(crate) fn path(&self, target: &PackageKey) -> Option<DependencyRequestPath> {
        self.closure.custody(target)?;
        let graph = self.closure.graph();
        let mut current = graph.package_position(target)?;
        let mut steps = Vec::new();
        while current != self.root_position {
            let (requester_position, edge_position) = self.predecessors[current]?;
            let requester = &graph.packages()[requester_position];
            let dependency = &requester.dependencies()[edge_position];
            steps.push(DependencyRequestPathStep {
                requester: requester.source().key().clone(),
                purpose: dependency.purpose(),
                dependency_index: dependency.dependency_index(),
                alias: dependency.alias().clone(),
                target: dependency.target().clone(),
            });
            current = requester_position;
        }
        steps.reverse();
        Some(DependencyRequestPath {
            root: graph.root().clone(),
            steps,
        })
    }
}
