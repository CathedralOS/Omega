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

/// Projection identifies a subtree, but grants no ownership of a borrowed
/// referent. Pattern payloads and ordinary fields obey the same reference-prefix
/// checks; moving a reference carrier itself does not move its referent.
pub(super) fn is_borrowed_place_transfer(
    program: &typed_trees::TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    let statement_index = event_statement_index(event.source).unwrap_or(0);
    let place = crate::flow::CanonicalPlace {
        root: event.root,
        segments: path.to_vec(),
    };
    let Some(projected_type) =
        crate::flow::canonical_place_type_reference(program, state.symbol, statement_index, &place)
    else {
        return false;
    };
    if type_multiplicity(program, projected_type) == Multiplicity::Unrestricted
        || type_reference_is_reference(program, projected_type)
    {
        return false;
    }
    // Self event roots are normalized to the machine; recover its actual
    // receiver declaration rather than interpreting the machine as a value.
    if let facts::PlaceRoot::Symbol(root) = event.root
        && root == machine
        && program.state_parameters(state).iter().any(|parameter| {
            parameter.is_self && type_reference_is_reference(program, parameter.type_reference)
        })
    {
        return true;
    }
    // A reference may occur at the root or inside an owned wrapper. Copying
    // or moving that reference does not permit taking its referent's value.
    (0..path.len()).any(|length| {
        let prefix = crate::flow::CanonicalPlace {
            root: event.root,
            segments: path[..length].to_vec(),
        };
        crate::flow::canonical_place_type_reference(program, state.symbol, statement_index, &prefix)
            .is_some_and(|reference| type_reference_is_reference(program, reference))
    })
}
