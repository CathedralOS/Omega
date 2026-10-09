//! Join receiving carrier occurrences to their historical supplying loans.
//! Selection belongs to the checked value graph; declared paths identify the
//! exact loan leaves. This module owns collection, formation choice and the
//! complete receiver-to-supplier correspondence used by retirement.

use super::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan, Results,
    SymbolHandle,
};
use checked_trees::BorrowLoanFact;

mod paths;
mod sources;

/// Historical carrier transport, not a new loan descent. A copy and an owned
/// move retain the supplying occurrence's immediate parents; neither makes
/// the supplying descriptor itself a parent of the receiving descriptor.
pub(in crate::execution::terminal_unit::control::statement_sequence) type TransportedLoans =
    Vec<(arena::Handle<BorrowLoanFact>, arena::Handle<BorrowLoanFact>)>;

/// Rejoin each receiving loan leaf through its retained construction graph.
/// The relation is prepared once for statement scheduling and remains valid
/// after the supplying holder retires: retirement cannot erase provenance.
pub(in crate::execution::terminal_unit::control::statement_sequence) fn collect(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    results: &Results,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<TransportedLoans> {
    let mut states = facts.borrow.states.iter().filter_map(|(_, borrowing)| {
        (borrowing.machine_symbol == machine && borrowing.state_symbol == state.symbol)
            .then_some(borrowing)
    });
    let Some(borrowing) = states.next() else {
        return Some(Vec::new());
    };
    if states.next().is_some() {
        return None;
    }
    let mut output = Vec::new();
    for operation in operations {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } =
            operation
        else {
            continue;
        };
        let mut registered = results.iter().filter(|(binding, _)| binding == result);
        let Some((_, facts::PlaceRoot::Symbol(owner))) = registered.next() else {
            continue;
        };
        if registered.next().is_some() {
            return None;
        }
        let (_, local) = program.state_local_declaration(state, *owner)?;
        for (receiver, loan) in facts.borrow.loans.iter().filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrowing, *handle)
                && loan.owner_symbol == *owner
                && loan.statement_index == result.statement_index as usize
        }) {
            let path = paths::carrier_path(
                program,
                local.type_reference,
                facts.borrow.loan_owner_path(loan),
            )?;
            let mut sources = Vec::new();
            sources::select(program, facts, *value, &path, &mut Vec::new(), &mut sources)?;
            for source in sources {
                let (source_owner, source_formation) = match source.source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        let mut holders = results
                            .iter()
                            .filter(|(binding, _)| binding.binding_ordinal == binding_ordinal);
                        let (binding, facts::PlaceRoot::Symbol(owner)) = holders.next()? else {
                            return None;
                        };
                        if holders.next().is_some() {
                            return None;
                        }
                        (*owner, Some(binding.statement_index as usize))
                    }
                    _ => match source.structural_storage_root() {
                        Some(facts::PlaceRoot::Symbol(owner)) => (owner, None),
                        // An ingress carrier has no retiring local parent in
                        // this activation. Its lifetime stays with its caller.
                        _ => continue,
                    },
                };
                let Some((_, supplying_local)) =
                    program.state_local_declaration(state, source_owner)
                else {
                    continue;
                };
                let mut supplying = Vec::new();
                for (handle, candidate) in
                    facts.borrow.loans.iter().filter(|(handle, candidate)| {
                        facts.borrow.state_owns_loan(borrowing, *handle)
                            && candidate.owner_symbol == source_owner
                            && candidate.statement_index < loan.statement_index
                            && source_formation
                                .is_none_or(|formation| candidate.statement_index == formation)
                    })
                {
                    if paths::carrier_path(
                        program,
                        supplying_local.type_reference,
                        facts.borrow.loan_owner_path(candidate),
                    )? == source.path
                    {
                        supplying.push((handle, candidate.statement_index));
                    }
                }
                // Replacements establish a new occurrence at the same slot.
                // Every alternative at that latest formation is retained.
                let Some(formation) = supplying.iter().map(|(_, statement)| *statement).max()
                else {
                    // Source borrow admission already owns this carrier. With
                    // no supplying local loan there is no local parent to
                    // inherit; absence must not select another slot's loan.
                    continue;
                };
                for (supplier, _) in supplying
                    .into_iter()
                    .filter(|(_, statement)| *statement == formation)
                {
                    if !output.contains(&(receiver, supplier)) {
                        output.push((receiver, supplier));
                    }
                }
            }
        }
    }
    Some(output)
}
