//! When two of a unit's structural places name one storage.
//!
//! The place-to-declaration half is `PsiOptimizationFunction::arrival_parameter`
//! on the owning type; these are the graph relations over it.

use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};

/// Candidate names for one use-site storage in the blocks behind that use.
/// Different arrivals can bind different owners to one formal. Membership is
/// therefore discovery only: an all-path freshness check must independently
/// establish the candidate's exact backing on every arrival.
pub(crate) struct StorageNames {
    behind: BTreeMap<BlockId, BTreeSet<PlaceId>>,
}

impl StorageNames {
    pub(crate) fn in_block(&self, block: BlockId) -> impl Iterator<Item = PlaceId> + '_ {
        self.behind.get(&block).into_iter().flatten().copied()
    }
}

/// Discover predecessor spellings through whole-storage bindings and ordinary
/// control edges. Retain every (block, place) state: two names can later converge
/// to one upstream owner, and dropping an ambiguous intermediate state leaves
/// previously propagated ancestor names stale.
pub(crate) fn storage_names(
    function: &PsiOptimizationFunction,
    block: BlockId,
    root: PlaceId,
) -> StorageNames {
    let mut behind = BTreeMap::<BlockId, BTreeSet<PlaceId>>::new();
    let mut pending = vec![(block, root)];
    while let Some((block, root)) = pending.pop() {
        if !behind.entry(block).or_default().insert(root) {
            continue;
        }
        for predecessor in &function.blocks {
            for edge in predecessor.successors() {
                if edge.target == block
                    && let Some(before) = root_before_edge(edge, root)
                {
                    pending.push((predecessor.id, before));
                }
            }
        }
    }
    StorageNames { behind }
}

/// The place that `edge`'s source block uses for the storage `place` names in
/// the edge's target. The edge must forward the whole storage; a projected
/// binding names only a part, so the caller must abandon the fact on that path
/// rather than carry it. A place the edge does not bind is not a parameter of
/// the target and already denotes one storage, so it answers itself.
pub(crate) fn root_before_edge(
    edge: &optimization_unit::OptimizationEdge,
    place: PlaceId,
) -> Option<PlaceId> {
    let (root, path) = projection_before_edge(edge, place);
    path.is_empty().then_some(root)
}

/// Retain a binding's complete projection. The binding's type, access and
/// availability are validated separately; this query grants no permission.
pub(crate) fn projection_before_edge(
    edge: &optimization_unit::OptimizationEdge,
    place: PlaceId,
) -> (PlaceId, &[terminal_psi::StructuralPathSegment]) {
    match edge.structural_binding(place) {
        Some(binding) => (binding.argument.place, &binding.argument.path),
        None => (place, &[]),
    }
}
