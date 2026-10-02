//! When two of a unit's structural places name one storage.
//!
//! The place-to-declaration half is `PsiOptimizationFunction::arrival_parameter`
//! on the owning type; these are the graph relations over it.

use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId};
use std::collections::BTreeMap;

/// The names one storage answers to, per block, among the blocks behind a use
/// of it, mirroring `terminal_semantics::storage_names` for the unit's graph.
///
/// A place denotes one storage wherever it is mentioned, so the difficulty is
/// never that a place changes meaning between blocks: it is that two different
/// places denote one storage, because every edge that forwards it renames it.
/// A receiver gives every state the one unit parameter, while a named data
/// parameter gives each state its own block parameter, so a search comparing
/// place identities finds an operation in another state only for the receiver
/// spelling. The use site's own place is therefore always an answer, and each
/// block behind the use may answer to one further place as well.
///
/// Membership says only that two places denote one storage, never that one
/// operation runs before another; callers that need that ordering still prove
/// it with the all-paths walk.
pub(crate) struct StorageNames {
    root: PlaceId,
    behind: BTreeMap<BlockId, Option<PlaceId>>,
}

impl StorageNames {
    /// The places `block` may use for this storage: the use site's own place,
    /// and the place this block forwards into it, when the two differ and this
    /// block reaches the use under exactly one such place.
    pub(crate) fn in_block(&self, block: BlockId) -> impl Iterator<Item = PlaceId> + '_ {
        std::iter::once(self.root).chain(
            self.behind
                .get(&block)
                .copied()
                .flatten()
                .filter(|place| *place != self.root),
        )
    }
}

/// The names the storage that `root` names inside `block` answers to in the
/// blocks that reach `block`.
///
/// Only predecessors are followed. An establishment is looked for behind the
/// use, never ahead of it, and naming a predecessor's argument is exact for
/// arrivals through that predecessor: the edge binds the two to one storage.
/// Following successors instead would name a target's parameter from one
/// incoming edge while another edge binds different storage into it, which no
/// later check would catch. Only whole-storage bindings are followed, for the
/// reason [`root_before_edge`] states.
pub(crate) fn storage_names(
    function: &PsiOptimizationFunction,
    block: BlockId,
    root: PlaceId,
) -> StorageNames {
    let mut behind = BTreeMap::new();
    behind.insert(block, Some(root));
    let mut changed = true;
    while changed {
        changed = false;
        for predecessor in &function.blocks {
            for edge in predecessor.nodes.iter().flat_map(|node| &node.successors) {
                let Some(&Some(known)) = behind.get(&edge.target) else {
                    continue;
                };
                for binding in &edge.structural_bindings {
                    if binding.parameter != known || !binding.argument.path.is_empty() {
                        continue;
                    }
                    changed |= learn(&mut behind, predecessor.id, binding.argument.place);
                }
            }
        }
    }
    StorageNames { root, behind }
}

/// Record one block-local name, erasing the entry to ambiguous when the block
/// already reaches the use under a different place: two names at once identify
/// no one storage, and the use site's own place remains the answer there.
fn learn(behind: &mut BTreeMap<BlockId, Option<PlaceId>>, block: BlockId, place: PlaceId) -> bool {
    match behind.get(&block) {
        Some(Some(known)) if *known == place => false,
        Some(None) => false,
        Some(Some(_)) => {
            behind.insert(block, None);
            true
        }
        None => {
            behind.insert(block, Some(place));
            true
        }
    }
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
    match edge
        .structural_bindings
        .iter()
        .find(|binding| binding.parameter == place)
    {
        Some(binding) => binding
            .argument
            .path
            .is_empty()
            .then_some(binding.argument.place),
        None => Some(place),
    }
}
