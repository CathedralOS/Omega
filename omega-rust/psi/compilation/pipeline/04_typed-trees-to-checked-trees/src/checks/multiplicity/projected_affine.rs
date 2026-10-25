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
    program: &dyn ::validation::CheckingInput,
    _facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    places: &[LinearPlace],
    remaining: &super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
) -> Result<(), diagnostics::Diagnostic> {
    let Some(statement_index) = event_statement_index(event.source) else {
        return Ok(());
    };
    // Payload binders retain their exact selected Case/Field coordinates.
    // Their affine storage moves even when only a sibling has a linear claim.
    if path.is_empty()
        || !path.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        })
        || program.state_by_symbol(state).is_none()
    {
        return Ok(());
    }
    // A carrier move is independent of its contained claim frontier. An
    // exact tracked endpoint already publishes its own transfer; otherwise
    // retain the storage path even when every payload claim is inactive.
    // A reserved root (a declared local its stores establish field by
    // field) has no established whole place; its construction frontier is
    // the authority on whether the moved path holds a value.
    let owners = places
        .iter()
        .filter(|place| place.root == event.root)
        .collect::<Vec<_>>();
    if owners.iter().any(|owner| owner.path == path)
        || !(owners.iter().any(|owner| owner.ever_established)
            || remaining.has_storage_frontier(event.root))
    {
        return Ok(());
    }
    let provenance = owners
        .iter()
        .find(|owner| owner.path.is_empty() && owner.multiplicity == Multiplicity::Affine)
        .and_then(|owner| owner.provenance)
        .unwrap_or(PermissionProvenance::Unknown);
    let root = validation::CanonicalPlace {
        root: event.root,
        segments: Vec::new(),
    };
    let projected = validation::CanonicalPlace {
        root: event.root,
        segments: path.to_vec(),
    };
    let affine = |place: &validation::CanonicalPlace| {
        validation::canonical_place_type_reference(program, state, statement_index, place)
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
        || program.state_by_symbol(state).is_none_or(|declaration| {
            is_borrowed_place_transfer(program, machine, declaration, event, path)
        })
    {
        return Ok(());
    }
    // A typed owned affine path still needs established storage. Absence
    // cannot silently omit its move event. Borrowed referents and copy
    // observations were excluded by the shared classification above.
    if !remaining.is_present(event.root, path) {
        if let Some(consumed) = remaining.consumed_place(event.root, path)
            && let Some(machine) = program.machine_by_symbol(machine)
            && let Some(state) = program.state_by_symbol(state)
        {
            let spelling =
                super::borrowed_windows::place_spelling(program, machine, state, event.root, path);
            let consumed_spelling = super::borrowed_windows::place_spelling(
                program,
                machine,
                state,
                consumed.root,
                &consumed.segments,
            );
            return Err(diagnostics::Diagnostic::error(
                super::consumed_owners::consumed_use_message(
                    machine.name.as_str(),
                    state.name.as_str(),
                    statement_index,
                    &spelling,
                    &consumed_spelling,
                    None,
                ),
            ));
        }
        return Err(diagnostics::Diagnostic::error(format!(
            "projected owned storage is not established in machine `{}` state `{}` at statement {}",
            program.symbols.name(machine),
            program.symbols.name(state),
            statement_index,
        )));
    }
    permissions.push(FlowPermissionEventFact {
        expression: event.expression,
        evaluation: checked_trees::FlowPermissionEvaluation::Statement,
        machine_symbol: machine,
        state_symbol: state,
        source: permission_source(event.source),
        kind: PermissionEventKind::Transfer,
        multiplicity: Multiplicity::Affine,
        access: PermissionAccess::Owned,
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance,
        root: event.root,
        segments: event.segments,
        obligation_live: false,
    });
    Ok(())
}

/// Copying an unrestricted value observes its source without detaching storage.
/// Affine reference carriers still transfer their descriptor custody, separately
/// from whether their referent is owned by this place.
pub(super) fn transfers_value_custody(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    selected_value_type(program, state, statement_index, event, path).is_some_and(|reference| {
        type_multiplicity(program, reference) != Multiplicity::Unrestricted
    })
}

/// Loan exclusion and borrowed-window opening distinguish an owned value from
/// a reference descriptor. Moving the descriptor does not detach its referent.
pub(in crate::checks) fn transfers_owned_storage(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    selected_value_type(program, state, statement_index, event, path).is_some_and(|reference| {
        type_multiplicity(program, reference) != Multiplicity::Unrestricted
            && !type_reference_is_reference(program, reference)
    })
}

fn selected_value_type(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    event
        .value_type
        .is_valid()
        .then_some(event.value_type)
        .or_else(|| {
            validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &validation::CanonicalPlace {
                    root: event.root,
                    segments: path.to_vec(),
                },
            )
        })
}

/// Projection identifies a subtree, but grants no ownership of a borrowed
/// referent. Pattern payloads and ordinary fields obey the same reference-prefix
/// checks; moving a reference carrier itself does not move its referent.
pub(super) fn is_borrowed_place_transfer(
    program: &dyn ::validation::CheckingInput,
    _machine: SymbolHandle,
    state: &typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    let statement_index = event_statement_index(event.source).unwrap_or(0);
    if !transfers_owned_storage(program, state.symbol, statement_index, event, path) {
        return false;
    }
    // Include the endpoint: consuming an exact referent can cross a reference
    // at an empty root path. A descriptor transfer was excluded above by its
    // selected value type, so this never turns forwarding into referent custody.
    has_reference_prefix(
        program,
        state.symbol,
        statement_index,
        &validation::CanonicalPlace {
            root: event.root,
            segments: path.to_vec(),
        },
    )
}

/// A reference prefix selects storage owned elsewhere, including the exact
/// referent at an empty root path. The selected value's carrier is classified
/// separately so forwarding a descriptor never becomes referent ownership.
pub(super) fn has_reference_prefix(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
) -> bool {
    (0..=place.segments.len()).any(|length| {
        let prefix = validation::CanonicalPlace {
            root: place.root,
            segments: place.segments[..length].to_vec(),
        };
        validation::canonical_place_type_reference(program, state, statement_index, &prefix)
            .is_some_and(|reference| type_reference_is_reference(program, reference))
    })
}
