//! A projected affine move transfers a subtree, not its still-live root.
use crate::checks::multiplicity::linear_obligations::LinearPlace;
use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
use crate::checks::multiplicity::linear_validation::event_statement_index;
use crate::checks::multiplicity::linear_validation::permission_source;
use crate::checks::type_multiplicity;
use checked_trees::CheckFacts;
use checked_trees::FlowPermissionEventFact;
use language_semantics::Multiplicity;
use language_semantics::PermissionAccess;
use language_semantics::PermissionClaimIdentity;
use language_semantics::PermissionEventKind;
use language_semantics::PermissionProvenance;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceNode;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_transfer(
    program: &typed_trees::TypedTrees,
    _facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    places: &[LinearPlace],
    remaining: &super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
) {
    let facts::PlaceRoot::Symbol(symbol) = event.root else {
        return;
    };
    let Some(statement_index) = event_statement_index(event.source) else {
        return;
    };
    if path.is_empty()
        || !path.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
        || crate::semantic::calls::find_state(program, state).is_none()
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
        || (!owner.live && !remaining.is_reserved(event.root))
        || !remaining.is_present(event.root, path)
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
    let affine = |place: &crate::flow::CanonicalPlace| {
        crate::flow::canonical_place_type_reference(program, state, statement_index, place)
            .is_some_and(|mut type_reference| {
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(type_reference)
                {
                    type_reference = *base_type;
                }
                matches!(
                    program.type_reference_table.type_reference(type_reference),
                    TypeReferenceNode::Named { .. }
                        | TypeReferenceNode::Generic { .. }
                        | TypeReferenceNode::FixedArray { .. }
                ) && type_multiplicity(program, type_reference) == Multiplicity::Affine
            })
    };
    if !affine(&root)
        || !affine(&projected)
        || crate::semantic::calls::find_state(program, state).is_none_or(|declaration| {
            is_borrowed_place_transfer(program, machine, declaration, event, path)
        })
    {
        return;
    }
    permissions.push(FlowPermissionEventFact {
        evaluation: checked_trees::FlowPermissionEvaluation::Statement,
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

/// A move candidate detaches owned storage only when its canonical endpoint
/// resolves to a non-copy value. Reference carriers transfer their permission,
/// while unresolved derived observations retain their existing owning checks.
/// Loan exclusion and borrowed-window opening must use the same judgment.
pub(in crate::checks) fn transfers_owned_storage(
    program: &typed_trees::TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    let place = crate::flow::CanonicalPlace {
        root: event.root,
        segments: path.to_vec(),
    };
    let Some(projected_type) =
        crate::flow::canonical_place_type_reference(program, state, statement_index, &place)
    else {
        return false;
    };
    if type_multiplicity(program, projected_type) == Multiplicity::Unrestricted
        || type_reference_is_reference(program, projected_type)
    {
        return false;
    }
    true
}

/// Projection identifies a subtree, but grants no ownership of a borrowed
/// referent. Pattern payloads and ordinary fields obey the same reference-prefix
/// checks; moving a reference carrier itself does not move its referent.
pub(super) fn is_borrowed_place_transfer(
    program: &typed_trees::TypedTrees,
    _machine: SymbolHandle,
    state: &typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    let statement_index = event_statement_index(event.source).unwrap_or(0);
    if !transfers_owned_storage(program, state.symbol, statement_index, event, path) {
        return false;
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

/// A projected affine store replaces one owned subtree, independently of
/// whether that subtree was extracted earlier. It does not establish or
/// consume the containing root's linear/affine claim.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_replacement(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    places: &[LinearPlace],
    remaining: &super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
) {
    let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
        return;
    };
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        assignment.target,
    ) else {
        return;
    };

    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return;
    };
    if place.segments.is_empty()
        || !place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return;
    }
    let Some(owner) = places.iter().find(|owner| {
        owner.symbol == symbol
            && owner.path.is_empty()
            && (owner.live || remaining.is_reserved(place.root))
            && owner.multiplicity == Multiplicity::Affine
    }) else {
        return;
    };
    // Crossing any reference changes storage ownership. Replacing the
    // borrowed carrier itself is also unrelated to nominal value cleanup.
    for length in 0..=place.segments.len() {
        let prefix = crate::flow::CanonicalPlace {
            root: place.root,
            segments: place.segments[..length].to_vec(),
        };
        let Some(reference) =
            crate::flow::canonical_place_type_reference(program, state, statement_index, &prefix)
        else {
            return;
        };
        if type_reference_is_reference(program, reference) {
            return;
        }
        if length == place.segments.len()
            && type_multiplicity(program, reference) == Multiplicity::Linear
        {
            return;
        }
    }
    let endpoint =
        crate::flow::canonical_place_type_reference(program, state, statement_index, &place);
    let affine = endpoint
        .is_some_and(|reference| type_multiplicity(program, reference) == Multiplicity::Affine);
    if !affine && !remaining.is_reserved(place.root) {
        // Copy-field stores carry no disposal event in a complete value.
        // They matter here only as establishment of reserved construction
        // storage; ordinary scalar write semantics stay with their owner.
        return;
    }
    let discarded = if affine {
        remaining.disposal_paths(place.root, &place.segments)
    } else {
        Vec::new()
    };
    let source = language_semantics::PermissionEventSource::Statement { statement_index };
    let segments = facts.flow.ownership.segments.insert_many(place.segments);
    for path in discarded {
        // Reserved storage may contain zero-valid copy fields alongside its
        // established owners. Completion needs both; disposal owes only the
        // selected value's actual multiplicity, never its parent's.
        let discarded = crate::flow::CanonicalPlace {
            root: place.root,
            segments: path,
        };
        if crate::flow::canonical_place_type_reference(program, state, statement_index, &discarded)
            .is_none_or(|reference| type_multiplicity(program, reference) != Multiplicity::Affine)
        {
            continue;
        }
        let discarded_segments = facts
            .flow
            .ownership
            .segments
            .insert_many(discarded.segments);
        permissions.push(FlowPermissionEventFact {
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            machine_symbol: machine,
            state_symbol: state,
            source,
            kind: PermissionEventKind::AffineDrop,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: owner.provenance.unwrap_or(PermissionProvenance::Unknown),
            root: place.root,
            segments: discarded_segments,
            obligation_live: false,
        });
    }
    permissions.push(FlowPermissionEventFact {
        evaluation: checked_trees::FlowPermissionEvaluation::Statement,
        machine_symbol: machine,
        state_symbol: state,
        source,
        kind: PermissionEventKind::Establish,
        multiplicity: if affine {
            Multiplicity::Affine
        } else {
            Multiplicity::Unrestricted
        },
        access: PermissionAccess::Owned,
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance: PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source,
        },
        root: place.root,
        segments,
        obligation_live: false,
    });
}
