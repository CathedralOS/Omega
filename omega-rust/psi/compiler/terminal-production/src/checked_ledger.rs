//! Production-side re-verification of the checked ownership ledger.
//!
//! Lowering consumes `facts.flow.ownership.permissions` as replay evidence, so
//! an event detached from its machine, a forged duplicate, or a return-site
//! transfer carrying a substituted obligation must be refused before an
//! artifact can be published from it.

use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::LoweringError;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};

/// Rejoin every recorded permission event to the machine and state it names,
/// then refuse malformed or duplicated ledger rows before lowering reads them.
pub(crate) fn verify(checked: &CheckedTrees) -> Result<(), LoweringError> {
    let ownership = &checked.facts.flow.ownership;
    let mut events = Vec::new();
    for (_, event) in ownership.permissions.iter() {
        let Some(machine) = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == event.machine_symbol)
        else {
            return Err(LoweringError::Unsupported(
                "checked permission event names no declared machine",
            ));
        };
        if !checked
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == event.state_symbol)
        {
            return Err(LoweringError::Unsupported(
                "checked permission event names no declared state of its machine",
            ));
        }
        if events.contains(&event) {
            return Err(LoweringError::Unsupported(
                "checked permission ledger repeats an identical event",
            ));
        }
        events.push(event);
        // An authored whole-statement move is exactly one discharged owned
        // transfer of an affine or linear root; a live obligation must name
        // the claim that stays open. Borrowed loans arrive as
        // Establish/Consume pairs and unrestricted data needs no permission,
        // so the authored transfer lane admits none of those shapes.
        if event.kind == PermissionEventKind::Transfer
            && matches!(event.source, PermissionEventSource::Statement { .. })
            && (event.access != PermissionAccess::Owned
                || event.multiplicity == Multiplicity::Unrestricted
                || (event.obligation_live
                    && event.claim_identity == PermissionClaimIdentity::Unknown))
        {
            return Err(LoweringError::Unsupported(
                "authored statement transfer is not a discharged owned affine/linear move",
            ));
        }
    }
    Ok(())
}
