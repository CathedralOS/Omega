//! Where each live owned linear transfer lands within the operand that
//! consumed it.
//!
//! Lowering binds a consuming operand to the claims it hands over. That needs
//! the operand's owning root, the operand's storage path under that root, and
//! each claim's path relative to that storage. All three are queries over the
//! immutable typed program and the final permission ledger, so checking answers
//! them once, after the flow fixed point seals the ledger, and publishes one
//! placement per event. The queries deliberately use the bare typed program,
//! without checking memos, deferred layout geometry or opaque selections:
//! for retained machines it answers exactly as the checked program's `typed`
//! field does, so a placement never depends on checking-only context. A
//! placement names storage only: the event keeps the claim identity,
//! evaluation lane and consuming coordinate.

use checked_trees::{
    CheckFacts, FlowOwnedTransferPath, FlowOwnedTransferPlacement, FlowPermissionEventFact,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};

type Arena<T> = arena::PagedArenaSet<T>;

pub(crate) fn publish_owned_transfer_paths(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) {
    let typed: &dyn ::validation::CheckingInput = program.typed_program();
    let ownership = &mut facts.flow.ownership;
    let mut paths = Arena::default();
    let mut rows = Arena::default();
    for (event_handle, event) in ownership.permissions.iter() {
        if !event.expression.is_valid()
            || event.kind != PermissionEventKind::Transfer
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Linear
            || !event.obligation_live
        {
            continue;
        }
        let placement = match event.source {
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            }
            | PermissionEventSource::InstructionDisposition { statement_index }
            | PermissionEventSource::InstructionExitDisposition { statement_index } => {
                transfer_placement(
                    typed,
                    event,
                    statement_index,
                    ownership.segments.span_or_empty(event.segments),
                    &mut paths,
                )
            }
            // An entry or exit event has no consuming statement to resolve
            // its operand in; lowering refuses rather than guessing one.
            _ => FlowOwnedTransferPlacement::Unresolved,
        };
        rows.append(FlowOwnedTransferPath {
            event: event_handle,
            placement,
        });
    }
    ownership.owned_transfer_paths = rows;
    ownership.owned_transfer_path_segments = paths;
}

fn transfer_placement(
    program: &dyn ::validation::CheckingInput,
    event: &FlowPermissionEventFact,
    statement: usize,
    claim: &[facts::PlaceSegment],
    paths: &mut Arena<checked_trees::CheckedUnitStructuralPathSegment>,
) -> FlowOwnedTransferPlacement {
    let state = event.state_symbol;
    // A noncopy saved subject's owning place is its original home, not its
    // read anchor; events on any other root belong to another operand.
    let Some(place) = ::validation::ownership_place_from_expression_in_state(
        program,
        state,
        statement,
        event.expression,
    ) else {
        return FlowOwnedTransferPlacement::Unresolved;
    };
    if place.root != event.root {
        return FlowOwnedTransferPlacement::OtherRoot;
    }
    let Some(root_type) = ::validation::canonical_place_type_reference(
        program,
        state,
        statement,
        &::validation::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        },
    ) else {
        return FlowOwnedTransferPlacement::Unresolved;
    };
    // The operand's coordinates name its storage, including a loan descriptor.
    // They own no referent; the claim path below must select linear custody.
    let Ok((storage, _)) =
        ::validation::structural_transfer_storage_path(program, root_type, &place.segments)
    else {
        return FlowOwnedTransferPlacement::Unresolved;
    };
    let Ok(path) = ::validation::structural_claim_path(program, root_type, claim) else {
        return FlowOwnedTransferPlacement::Unresolved;
    };
    match path.strip_prefix(storage.as_slice()) {
        Some(relative) => {
            FlowOwnedTransferPlacement::Within(paths.insert_many(relative.iter().cloned()))
        }
        None => FlowOwnedTransferPlacement::Outside,
    }
}
