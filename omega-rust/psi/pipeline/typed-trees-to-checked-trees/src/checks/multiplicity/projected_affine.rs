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

/// Case membership identifies the active payload, but grants no ownership of
/// a borrowed referent. A saved pattern subject must obey the same rule as an
/// explicitly authored member expression; no marker spelling participates.
pub(super) fn is_borrowed_case_transfer(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    if !path
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    {
        return false;
    }
    let statement_index = event_statement_index(event.source).unwrap_or(0);
    let place = crate::flow::CanonicalPlace {
        root: event.root,
        segments: path.to_vec(),
    };
    let Some(payload) =
        crate::flow::canonical_place_type_reference(program, state.symbol, statement_index, &place)
    else {
        return false;
    };
    if type_multiplicity(program, payload) == Multiplicity::Unrestricted
        || type_reference_is_reference(program, payload)
    {
        return false;
    }
    // Self event roots are normalized to the machine; recover its actual
    // receiver declaration rather than interpreting the machine as a value.
    if let facts::PlaceRoot::Symbol(root) = event.root
        && program.machines().iter().any(|machine| {
            machine.symbol == root
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|candidate| candidate.symbol == state.symbol)
        })
        && program.state_parameters(state).iter().any(|parameter| {
            parameter.is_self && type_reference_is_reference(program, parameter.type_reference)
        })
    {
        return true;
    }
    // A reference may occur at the root or inside an owned wrapper. Copying
    // or moving that reference does not permit taking its referent's payload.
    (0..path.len()).any(|length| {
        let prefix = crate::flow::CanonicalPlace {
            root: event.root,
            segments: path[..length].to_vec(),
        };
        crate::flow::canonical_place_type_reference(program, state.symbol, statement_index, &prefix)
            .is_some_and(|reference| type_reference_is_reference(program, reference))
    })
}
