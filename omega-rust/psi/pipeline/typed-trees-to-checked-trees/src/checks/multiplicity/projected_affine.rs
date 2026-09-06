//! A projected affine move transfers a subtree, not its still-live root.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_transfer(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    places: &[LinearPlace],
    permissions: &mut Vec<FlowPermissionEventFact>,
) {
    let facts::PlaceRoot::Symbol(symbol) = event.root else {
        return;
    };
    let FlowOwnershipEventSource::Call {
        statement_index, ..
    } = event.source
    else {
        return;
    };
    if path.is_empty()
        || !path.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
        || permission_kind_for_move(program, facts, machine, state, event)
            != PermissionEventKind::Transfer
        || crate::find_state(program, state).is_none_or(|state| {
            program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.symbol == symbol && parameter.is_self)
        })
    {
        return;
    }
    let mut owners = places.iter().filter(|place| place.symbol == symbol);
    let Some(owner) = owners.next() else {
        return;
    };
    if owners.next().is_some()
        || !owner.path.is_empty()
        || owner.multiplicity != Multiplicity::Affine
        || !owner.live
        || owner
            .claim_identity
            .is_some_and(|claim| claim != PermissionClaimIdentity::Unknown)
    {
        return;
    }
    let root = crate::flow::CanonicalPlace {
        root: event.root,
        segments: Vec::new(),
    };
    let projected = crate::flow::CanonicalPlace {
        root: event.root,
        segments: path.to_vec(),
    };
    let plain_affine = |place: &crate::flow::CanonicalPlace| {
        crate::flow::canonical_place_type_reference(program, state, statement_index, place)
            .is_some_and(|type_reference| {
                matches!(
                    program.type_reference_table.type_reference(type_reference),
                    TypeReferenceNode::Named { .. }
                        | TypeReferenceNode::Generic { .. }
                        | TypeReferenceNode::FixedArray { .. }
                ) && type_multiplicity(program, type_reference) == Multiplicity::Affine
                    && validation::has_plain_owned_contents(program, type_reference)
            })
    };
    if !plain_affine(&root) || !plain_affine(&projected) {
        return;
    }
    permissions.push(FlowPermissionEventFact {
        machine_symbol: machine,
        state_symbol: state,
        source: permission_source(event.source),
        kind: PermissionEventKind::Transfer,
        multiplicity: Multiplicity::Affine,
        access: PermissionAccess::Owned,
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance: owner.provenance.unwrap_or(PermissionProvenance::Unknown),
        root: event.root,
        segments: event.segments,
        obligation_live: false,
    });
}
