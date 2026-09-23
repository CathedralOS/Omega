//! Invocation-local root/offset homes; calls additionally materialize a descriptor.
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarTerminator};
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, ValueId};
use std::collections::BTreeSet;

use crate::SelectedInstructionError;

/// Each producer establishes offset + length <= root_length in mathematical
/// integers. The root's length is U64, so subsequent offset sums are exact U64.
/// Construction and replay establish this invariant separately from the admitted
/// source chain; this record itself grants no proof or memory-read authority.
///
/// `root`/`root_length` and `source` retain the storage identity the access
/// roster charges: a byte payload row emitted for a view access names the
/// view's storage root (or each bound root for a block-parameter view), never
/// the view's own `place`. Without that binding the roster would compare
/// `Place(view)` against `Place(root)`, reading differing place identities as
/// disjoint storage — a distinct-place non-aliasing premise nothing retained.
#[derive(Clone, Copy)]
pub(super) struct ByteViewHomes {
    pub place: PlaceId,
    /// The place this view was cut from: the `subslice` `source` operand.
    /// Consulted to enumerate bound roots when `root` is dynamic.
    pub source: PlaceId,
    /// The retained storage root — the place whose byte reach the view's
    /// payload physically occupies. `None` when the view was cut from a
    /// block-parameter view whose backing arrives over each incoming edge's
    /// structural bindings; `view_backing_roots` resolves that dynamic reach
    /// at access time.
    pub root: Option<PlaceId>,
    pub backing_pointer: VirtualRegisterId,
    pub byte_offset: VirtualRegisterId,
    pub byte_length: VirtualRegisterId,
    /// The extent bound retained for the storage root: the root's own byte
    /// length when `root` is named statically, else the immediate source's
    /// checked length — the tightest bound the cut verified.
    pub root_length: ValueId,
}

/// `Some(place)` when `place` names its own storage root; `None` when the
/// place is a block-parameter view whose backing arrives over edge bindings.
/// A place undeclared in the contract cannot be a block parameter, so it is
/// its own root.
pub(super) fn own_view_root(source: &LegalizedScalarFunction, place: PlaceId) -> Option<PlaceId> {
    match source
        .structural
        .as_ref()
        .and_then(|contract| contract.structural_places.iter().find(|d| d.id == place))
        .map(|declaration| declaration.kind)
    {
        Some(StructuralPlaceKind::BlockParameter { .. }) => None,
        _ => Some(place),
    }
}

/// The storage roots a byte payload access on `place` physically reaches.
///
/// `Ok(None)` means `place` is not a byte-sequence view, so the caller keeps
/// its ordinary place row. `Some` pairs each carry a `(root, extent)` the
/// roster row must name: a view cut from statically backed storage yields its
/// retained root; a view whose backing arrives over edge bindings (a
/// block-parameter view, or a view cut from one) yields the union of roots the
/// incoming edges bind to that parameter.
///
/// Projected binding paths, an incoming edge that binds nothing, and bound
/// places whose own backing is again dynamic all fail closed. Source
/// admission and independent replay share this enumeration, so both either
/// emit identical root rows or both reject.
pub(super) fn view_backing_roots(
    source: &LegalizedScalarFunction,
    views: &[ByteViewHomes],
    place: PlaceId,
    extent: ValueId,
) -> Result<Option<Vec<(PlaceId, ValueId)>>, SelectedInstructionError> {
    if let Some(view) = views.iter().find(|view| view.place == place) {
        return match view.root {
            Some(root) => Ok(Some(vec![(root, view.root_length)])),
            // The view's own backing is a block-parameter view: enumerate the
            // places every incoming edge binds to its `source` parameter.
            None => bound_view_roots(source, views, view.source, view.root_length, &mut BTreeSet::new())
                .map(Some),
        };
    }
    if crate::selection::established_view_input::view_type(source, place).is_none() {
        return Ok(None);
    }
    let Some(contract) = source.structural.as_ref() else {
        return Ok(None);
    };
    if !contract.structural_places.iter().any(|declaration| {
        declaration.id == place
            && matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. })
    }) {
        return Ok(None);
    }
    bound_view_roots(source, views, place, extent, &mut BTreeSet::new()).map(Some)
}

/// The distinct roots every incoming edge binds to block parameter
/// `parameter`. A bound byte-view place contributes its own retained root; a
/// bound un-sliced parameter or literal contributes itself with `extent`, the
/// checked length the access's operand already carries. A bound argument that
/// is itself a block parameter contributes the roots its own incoming edges
/// bind, enumerated transitively — a cycle back to a parameter already under
/// enumeration adds no roots, since its static backing arrives through other
/// edges. Anything else — a projected binding path or an incoming edge that
/// binds nothing — leaves the reach unjustified.
fn bound_view_roots(
    source: &LegalizedScalarFunction,
    views: &[ByteViewHomes],
    parameter: PlaceId,
    extent: ValueId,
    visiting: &mut BTreeSet<PlaceId>,
) -> Result<Vec<(PlaceId, ValueId)>, SelectedInstructionError> {
    if !visiting.insert(parameter) {
        return Ok(Vec::new());
    }
    let roots = bound_view_roots_recurse(source, views, parameter, extent, visiting);
    visiting.remove(&parameter);
    roots
}

fn bound_view_roots_recurse(
    source: &LegalizedScalarFunction,
    views: &[ByteViewHomes],
    parameter: PlaceId,
    extent: ValueId,
    visiting: &mut BTreeSet<PlaceId>,
) -> Result<Vec<(PlaceId, ValueId)>, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let Some(contract) = source.structural.as_ref() else {

        return Err(invalid());
    };
    let Some(declaration) = contract
        .structural_places
        .iter()
        .find(|declaration| declaration.id == parameter)
    else {

        return Err(invalid());
    };
    let StructuralPlaceKind::BlockParameter { block, .. } = declaration.kind else {

        return Err(invalid());
    };
    let bound_root = |place: PlaceId,
                      visiting: &mut BTreeSet<PlaceId>|
     -> Result<Vec<(PlaceId, ValueId)>, SelectedInstructionError> {
        if let Some(bound_view) = views.iter().find(|view| view.place == place) {
            // A bound subslice contributes its own root bound: its bytes sit at
            // `O + i` inside `root`, so the honest reach is `0 .. root_length`.
            return bound_view
                .root
                .map(|root| vec![(root, bound_view.root_length)])
                .ok_or_else(invalid);
        }
        let kind = contract
            .structural_places
            .iter()
            .find(|declaration| declaration.id == place)
            .map(|declaration| declaration.kind);
        match kind {
            // A bound un-sliced place is its own root; the parameter's checked
            // length is the bound the descriptor transport preserves. An
            // operation-result place's bytes live in storage the function owns,
            // so it roots its own reach the same way. (Subslice results were
            // already claimed by `views` above.)
            Some(
                StructuralPlaceKind::Parameter { .. }
                | StructuralPlaceKind::ByteSequenceLiteral { .. }
                | StructuralPlaceKind::OperationResult { .. },
            ) => Ok(vec![(place, extent)]),
            // A bound place whose backing is itself edge-decided contributes
            // the roots bound to it transitively.
            Some(StructuralPlaceKind::BlockParameter { .. }) => {
                bound_view_roots(source, views, place, extent, visiting)
            }
            // An undeclared or otherwise dynamic place keeps the reach
            // unjustified.
            _ => Err(invalid()),
        }
    };
    let mut bound_edges = 0usize;
    let mut unbound_edges = 0usize;
    let mut roots: Vec<(PlaceId, ValueId)> = Vec::new();
    for owner in &source.blocks {
        for (target, successor) in edge_successors(&owner.terminator) {
            if target != block {
                continue;
            }
            // Case edges carry no structural bindings; only jump and
            // conditional successors can bind a block parameter.
            let Some(successor) = successor else {
                unbound_edges += 1;
                continue;
            };
            let mut bound = false;
            for binding in &successor.structural_bindings {
                if binding.parameter != parameter {
                    continue;
                }
                if binding.argument.path.is_empty() {
                    bound = true;
                    for candidate in bound_root(binding.argument.place, visiting)? {
                        if !roots.contains(&candidate) {
                            roots.push(candidate);
                        }
                    }
                }
            }
            if bound {
                bound_edges += 1;
            } else {
                // A projected path or no binding at all leaves this edge's
                // reach unnamed.
                unbound_edges += 1;
            }
        }
    }
    if bound_edges == 0 || unbound_edges != 0 || roots.is_empty() {

        return Err(invalid());
    }
    Ok(roots)
}

/// Every edge leaving `terminator`, as `(target, bindings)`: `Some` for the
/// jump/conditional successors that can bind structural parameters, `None`
/// for case successors that carry only scalar payloads.
fn edge_successors(
    terminator: &LegalizedScalarTerminator,
) -> Vec<(
    semantic_vocabulary::BlockId,
    Option<&legalized_operations::LegalizedScalarSuccessor>,
)> {
    match terminator {
        LegalizedScalarTerminator::Jump { successor, .. } => {
            vec![(successor.target, Some(successor))]
        }
        LegalizedScalarTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![
            (when_true.target, Some(when_true)),
            (when_false.target, Some(when_false)),
        ],
        LegalizedScalarTerminator::StructuralCase { cases, .. } => {
            cases.iter().map(|case| (case.target, None)).collect()
        }
        _ => Vec::new(),
    }
}
