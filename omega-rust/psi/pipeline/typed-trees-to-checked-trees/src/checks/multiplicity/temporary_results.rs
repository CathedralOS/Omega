//! A partial move from a temporary has no remaining local owner in which to
//! retain unselected linear claims. The selected path must carry every live
//! obligation; ordinary affine siblings may be discarded.

use super::*;

pub(super) fn check_unselected_claims(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let facts::PlaceRoot::Expression(_) = event.root else {
        return;
    };
    let root = crate::flow::CanonicalPlace {
        root: event.root,
        segments: Vec::new(),
    };
    let Some(reference) = crate::flow::canonical_place_type_reference(
        program,
        state_symbol,
        event_statement_index(event.source).unwrap_or(0),
        &root,
    ) else {
        return;
    };
    for claim in linear_claim_frontier(program, reference) {
        if claim.multiplicity != Multiplicity::Linear
            || claim.path.starts_with(path)
            || claim_paths_are_case_alternatives(&claim.path, path)
        {
            continue;
        }
        diagnostics.push(Diagnostic::error(
            "cannot partially move a temporary call result while an unselected linear claim remains; bind the result to a local and transfer or consume every claim",
        ));
        break;
    }
}
