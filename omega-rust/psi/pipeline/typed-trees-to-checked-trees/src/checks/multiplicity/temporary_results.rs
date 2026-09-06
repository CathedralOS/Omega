//! Ownership transfers from temporary call results.
//!
//! A partial move from a temporary has no remaining local owner in which to
//! retain unselected linear claims. The selected path must carry every live
//! obligation; ordinary affine siblings may be discarded.

use super::*;

pub(super) fn append_whole_affine_transfer(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    calls: &[checked_trees::FlowCallFact],
    event: &crate::flow::DiscoveredMoveEvent,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    let facts::PlaceRoot::Expression(expression) = event.root else {
        return;
    };
    let FlowOwnershipEventSource::Call {
        statement_index, ..
    } = event.source
    else {
        return;
    };
    // Only the whole result of an actually discovered ordinary call
    // participates here. Projected results and reference/qualified or
    // claim-carrying roots retain their existing permission rules.
    let ordinary_affine_result = event.segments.is_empty()
        && calls.iter().any(|call| {
            call.statement_index == statement_index
                && call.authored_expression == expression
                && crate::find_state(program, call.target_symbol).is_some_and(|target| {
                    matches!(
                        program
                            .type_reference_table
                            .type_reference(target.return_type),
                        TypeReferenceNode::Named { .. }
                            | TypeReferenceNode::Generic { .. }
                            | TypeReferenceNode::FixedArray { .. }
                    ) && type_multiplicity(program, target.return_type) == Multiplicity::Affine
                        && !type_carries_linear_obligation(program, target.return_type)
                })
        });
    if ordinary_affine_result {
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: permission_source(event.source),
            kind: PermissionEventKind::Transfer,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: PermissionProvenance::Unknown,
            root: event.root,
            segments: HandleSpan::empty(),
            obligation_live: false,
        });
    }
}

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
