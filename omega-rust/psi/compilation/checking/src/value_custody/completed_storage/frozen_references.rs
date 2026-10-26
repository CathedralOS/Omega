//! Pure observation coordinates of a retained reference leaf.
//! This authenticates the complete copied inventory and typed endpoint only;
//! borrowers separately authorize its formation, namespace and live loans.
use arena::Handle;
use checked_trees::FlowOperandFact;

pub enum FrozenReferenceSelection {
    NotFrozen,
    Endpoint {
        owner: FlowOperandFact,
        leaf: checked_trees::FlowCapturedReferenceLeaf,
        path: Vec<facts::PlaceSegment>,
    },
    /// A unique valid inline descriptor precedes the requested endpoint.
    /// Its later pointer crossing belongs to the current-snapshot resolver.
    ThroughReference,
    Unavailable,
}

/// Select observation coordinates, never a permission. Missing or malformed
/// inventories must not fall through to an ordinary descriptor/source shortcut.
pub fn frozen_reference_leaf(
    program: &dyn crate::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    operand: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
) -> FrozenReferenceSelection {
    use FrozenReferenceSelection::{Endpoint, NotFrozen, ThroughReference, Unavailable};
    if !control.operands.is_valid(operand) {
        return Unavailable;
    }
    let capture = *control.operands.get(operand);
    let (owner, leaves, prefix, path) = match capture.completion {
        checked_trees::FlowOperandCompletion::Copied { copy, path } => {
            if !control.copied_values.is_valid(copy) {
                return Unavailable;
            }
            let copied = control.copied_values.get(copy);
            if !control.operands.is_valid(copied.source) {
                return Unavailable;
            }
            let Some(prefix) = control.operand_child_paths.span(path) else {
                return Unavailable;
            };
            let mut path = prefix.to_vec();
            path.extend_from_slice(projection);
            (
                *control.operands.get(copied.source),
                copied.references,
                prefix.len(),
                path,
            )
        }
        _ if !capture.reference_leaves.is_empty() => {
            (capture, capture.reference_leaves, 0, projection.to_vec())
        }
        _ => return NotFrozen,
    };
    let Some(leaves) = control.captured_reference_leaves.span(leaves) else {
        return Unavailable;
    };
    let mut selected = None;
    for leaf in leaves {
        let Some(leaf_path) = control.operand_child_paths.span(leaf.path) else {
            return Unavailable;
        };
        if leaf_path.len() <= path.len()
            && leaf_path.iter().zip(&path).all(|(left, right)| {
                crate::storage_path_relations::canonical_place_segments_equal(
                    program, *left, *right,
                )
            })
        {
            if selected.replace((*leaf, leaf_path)).is_some() {
                return Unavailable;
            }
        }
    }
    let Some((leaf, leaf_path)) = selected else {
        return Unavailable;
    };
    if leaf_path.len() < prefix {
        return Unavailable;
    }
    let Some(selected) =
        crate::project_type_reference_from_segments(program, owner.type_reference, leaf_path)
    else {
        return Unavailable;
    };
    let (Some(actual), Some(declared)) = (
        crate::reference_shell(program, selected),
        crate::reference_shell(program, leaf.type_reference),
    ) else {
        return Unavailable;
    };
    if actual.access != declared.access
        || !crate::storage_origin_types_match(program, actual.referee, declared.referee)
    {
        return Unavailable;
    }
    if leaf_path.len() < path.len() {
        // A typed final descriptor and a nonempty suffix establish that this
        // query really continues beyond the frozen leaf. Geometry/access for
        // that crossing remain the existing shared resolver's obligation.
        if !crate::project_type_reference_from_segments(program, owner.type_reference, &path)
            .is_some_and(|reference| crate::reference_shell(program, reference).is_some())
        {
            return Unavailable;
        }
        return ThroughReference;
    }
    Endpoint { owner, leaf, path }
}
