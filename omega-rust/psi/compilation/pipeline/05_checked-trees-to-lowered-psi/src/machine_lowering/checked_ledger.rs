//! Lowering-input validation of the checked ownership ledger.
//!
//! Lowering consumes `facts.flow.ownership.permissions` as replay evidence, so
//! an event detached from its machine, a forged duplicate, or a return-site
//! transfer carrying a substituted obligation must be refused before an
//! artifact can be published from it.

use crate::LoweringError;
use checked_trees::CheckedTrees;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
};

/// Rejoin every recorded permission event to the machine and state it names,
/// then refuse malformed or duplicated ledger rows before lowering reads them.
pub(crate) fn verify(checked: &CheckedTrees) -> Result<(), LoweringError> {
    let ownership = &checked.facts.flow.ownership;
    // The ledger is verified once over every event, so both of its lookups
    // read declaration indices rather than scanning the program per event,
    // and duplicate detection is a set membership rather than a linear
    // `contains` over everything seen so far.
    let mut seen = std::collections::HashSet::with_capacity(ownership.permissions.len());
    for (_, event) in ownership.permissions.iter() {
        // Product pruning retains a machine's permission rows as evidence
        // after the machine declaration leaves the surface; the row still
        // names a declared machine symbol, so only a forged handle — which
        // resolves to no machine declaration at all — rejects.
        match checked.machine_by_symbol(event.machine_symbol) {
            Some(machine) => {
                if checked
                    .state_position_by_symbol(machine.symbol, event.state_symbol)
                    .is_none()
                {
                    return Err(LoweringError::Unsupported(
                        "checked permission event names no declared state of its machine",
                    ));
                }
            }
            None => {
                if checked.typed.symbols.get(event.machine_symbol).kind
                    != symbols::SymbolKind::Machine
                {
                    return Err(LoweringError::Unsupported(
                        "checked permission event names no declared machine",
                    ));
                }
            }
        }
        if !seen.insert(event) {
            return Err(LoweringError::Unsupported(
                "checked permission ledger repeats an identical event",
            ));
        }
        // A live obligation always names the claim that stays open: entry
        // places mint one, written destinations mint one, and a linear
        // handoff keeps its claim across a call boundary. Loans settle as
        // Establish/Consume pairs and discharged affine moves carry no
        // obligation at all, so this holds for every kind at every source.
        if event.obligation_live && event.claim_identity == PermissionClaimIdentity::Unknown {
            return Err(LoweringError::Unsupported(
                "checked permission event carries a live obligation without an identified claim",
            ));
        }
        // An owned custody move is one transfer of an affine or linear root.
        // Borrowed loans arrive as Establish/Consume pairs and unrestricted
        // data needs no permission, so the transfer lane admits neither at
        // authored statements, call sites, or machine edges alike.
        if event.kind == PermissionEventKind::Transfer
            && (event.access != PermissionAccess::Owned
                || event.multiplicity == Multiplicity::Unrestricted)
        {
            return Err(LoweringError::Unsupported(
                "checked permission transfer is not an owned affine/linear move",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CheckedTrees, LoweringError, verify};
    use crate::front_end::checked_program as checked;
    use checked_trees::flow::FlowPermissionEventFact;
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
    };
    use symbols::SymbolHandle;

    const SOURCE: &str = "data Record { first: u64; second: u64; }
         machine retain(record: Record) -> Record { record }";

    #[test]
    fn direct_lowering_rejects_forged_permission_owners() {
        use crate::{TerminalMachineSelection, lower_machine};

        let mut checked = checked("machine run() -> u64 { 17 }");
        lower_machine(&checked, TerminalMachineSelection::Name("run"))
            .expect("the unmodified scalar program lowers");
        checked
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol: SymbolHandle::invalid(),
                ..FlowPermissionEventFact::default()
            });

        assert_eq!(
            lower_machine(&checked, TerminalMachineSelection::Name("run")).unwrap_err(),
            LoweringError::Unsupported("checked permission event names no declared machine"),
        );
    }

    // A well-formed owned move at a declared machine/state: each test swaps a
    // single field to isolate the rejection it exercises.
    fn forged_event(
        checked: &CheckedTrees,
        kind: PermissionEventKind,
        source: PermissionEventSource,
    ) -> FlowPermissionEventFact {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| !checked.machine_states(machine).is_empty())
            .expect("the fixture declares a machine with a state");
        FlowPermissionEventFact {
            machine_symbol: machine.symbol,
            state_symbol: checked.machine_states(machine)[0].symbol,
            source,
            kind,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            ..FlowPermissionEventFact::default()
        }
    }

    #[test]
    fn verify_rejects_an_event_naming_no_declared_machine() {
        let mut checked = checked(SOURCE);
        let (handle, _) = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .next()
            .expect("the fixture records a permission event");
        checked
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(handle)
            .machine_symbol = SymbolHandle::invalid();
        assert_eq!(
            verify(&checked),
            Err(LoweringError::Unsupported(
                "checked permission event names no declared machine"
            ))
        );
    }

    #[test]
    fn verify_rejects_an_event_naming_no_declared_state() {
        let mut checked = checked(SOURCE);
        let (handle, _) = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .next()
            .expect("the fixture records a permission event");
        checked
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(handle)
            .state_symbol = SymbolHandle::invalid();
        assert_eq!(
            verify(&checked),
            Err(LoweringError::Unsupported(
                "checked permission event names no declared state of its machine"
            ))
        );
    }

    #[test]
    fn verify_rejects_an_identical_repeated_event() {
        let mut checked = checked(SOURCE);
        let event = forged_event(
            &checked,
            PermissionEventKind::Transfer,
            PermissionEventSource::Statement { statement_index: 0 },
        );
        checked
            .facts
            .flow
            .ownership
            .permissions
            .insert(event.clone());
        checked.facts.flow.ownership.permissions.insert(event);
        assert_eq!(
            verify(&checked),
            Err(LoweringError::Unsupported(
                "checked permission ledger repeats an identical event"
            ))
        );
    }

    #[test]
    fn verify_rejects_a_statement_transfer_that_is_not_an_owned_move() {
        for access in [PermissionAccess::Shared, PermissionAccess::Exclusive] {
            let mut checked = checked(SOURCE);
            let mut event = forged_event(
                &checked,
                PermissionEventKind::Transfer,
                PermissionEventSource::Statement { statement_index: 0 },
            );
            event.access = access;
            checked.facts.flow.ownership.permissions.insert(event);
            assert_eq!(
                verify(&checked),
                Err(LoweringError::Unsupported(
                    "checked permission transfer is not an owned affine/linear move"
                )),
                "{access:?}"
            );
        }
        let mut checked = checked(SOURCE);
        let mut event = forged_event(
            &checked,
            PermissionEventKind::Transfer,
            PermissionEventSource::Statement { statement_index: 0 },
        );
        event.multiplicity = Multiplicity::Unrestricted;
        checked.facts.flow.ownership.permissions.insert(event);
        assert_eq!(
            verify(&checked),
            Err(LoweringError::Unsupported(
                "checked permission transfer is not an owned affine/linear move"
            ))
        );
    }

    #[test]
    fn verify_rejects_a_call_transfer_that_is_not_an_owned_move() {
        let mut checked = checked(SOURCE);
        let mut event = forged_event(
            &checked,
            PermissionEventKind::Transfer,
            PermissionEventSource::Call {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: SymbolHandle::invalid(),
            },
        );
        event.access = PermissionAccess::Shared;
        checked.facts.flow.ownership.permissions.insert(event);
        assert_eq!(
            verify(&checked),
            Err(LoweringError::Unsupported(
                "checked permission transfer is not an owned affine/linear move"
            ))
        );
    }

    #[test]
    fn verify_rejects_a_live_obligation_without_an_identified_claim() {
        for (kind, source) in [
            (
                PermissionEventKind::Establish,
                PermissionEventSource::StateEntry,
            ),
            (
                PermissionEventKind::Consume,
                PermissionEventSource::StateExit,
            ),
            (
                PermissionEventKind::Transfer,
                PermissionEventSource::Call {
                    statement_index: 0,
                    call_ordinal: 0,
                    target_symbol: SymbolHandle::invalid(),
                },
            ),
            (
                PermissionEventKind::AffineDrop,
                PermissionEventSource::Statement { statement_index: 0 },
            ),
        ] {
            let mut checked = checked(SOURCE);
            let mut event = forged_event(&checked, kind, source);
            event.obligation_live = true;
            checked.facts.flow.ownership.permissions.insert(event);
            assert_eq!(
                verify(&checked),
                Err(LoweringError::Unsupported(
                    "checked permission event carries a live obligation without an identified claim"
                )),
                "{kind:?} at {source:?}"
            );
        }
    }
}
