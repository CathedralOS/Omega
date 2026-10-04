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
