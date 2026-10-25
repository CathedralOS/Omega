//! Join receiving carrier occurrences to their historical supplying loans.
//! Selection belongs to the checked value graph; declared paths identify the
//! exact loan leaves. This module owns collection, formation choice and the
//! complete receiver-to-supplier correspondence used by retirement.

use super::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan, Results,
    SymbolHandle,
};
use checked_trees::BorrowLoanFact;

pub(super) use crate::borrow::carrier_path;

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
    let Some(borrowing) = trace_query(
        machine,
        state.symbol,
        "unique-borrow-state",
        super::unique_borrow_state(facts, machine, state.symbol),
        &(),
    )?
    else {
        return Some(Vec::new());
    };
    if borrowing.loans.is_empty() {
        return Some(Vec::new());
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let mut output = Vec::new();
    // Receiving occurrences exist independently of materialized results. In
    // particular a Unit call can preserve or replace a stored descriptor.
    // Consume its exact supplier edges from the checked state roster before
    // adding relationships recovered from retained constructor value graphs.
    for (receiver, loan) in trace_query(
        machine,
        state.symbol,
        "loan-group",
        facts.borrow.loan_group(borrowing.loans),
        &borrowing.loans,
    )? {
        for supplier in trace_query(
            machine,
            state.symbol,
            "supplier-roster",
            facts.borrow.loan_suppliers.span(loan.supplying_loans),
            &(receiver, loan.supplying_loans),
        )? {
            // Checking published this historical supplier from the receiver's
            // exact predecessor roster. Several ordered invocations can share
            // one authored statement, so statement precedence cannot replay
            // that formation judgment. Retain the published edge itself;
            // state membership still authenticates both occurrence handles.
            if !facts.borrow.state_owns_loan(borrowing, *supplier) {
                return trace_query(
                    machine,
                    state.symbol,
                    "supplier-state-membership",
                    None,
                    &(receiver, supplier),
                );
            }
            if !output.contains(&(receiver, *supplier)) {
                output.push((receiver, *supplier));
            }
        }
    }
    for operation in operations {
        let (owner, formation) =
            if let Some((result, _)) = operation.discardable_structural_result() {
                let mut registered = results.iter().filter(|(binding, _)| binding == result);
                let Some((_, facts::PlaceRoot::Symbol(owner))) = registered.next() else {
                    continue;
                };
                if registered.next().is_some() {
                    return trace_query(
                        machine,
                        state.symbol,
                        "duplicate-result-owner",
                        None,
                        &result,
                    );
                }
                (*owner, result.statement_index)
            } else if let Some((owner, formation, _)) =
                super::stores::receiver(program, facts, machine, state, operation)
            {
                (owner, formation)
            } else {
                continue;
            };
        let reference = trace_query(
            machine,
            state.symbol,
            "receiver-storage-type",
            validation::reference_result_custody::storage_owner_type_reference(
                program, state, owner, formation,
            ),
            &(owner, formation),
        )?;
        for (receiver, loan) in trace_query(
            machine,
            state.symbol,
            "receiver-resource-loans",
            facts.borrow.statement_resource_loans(borrowing),
            &(owner, formation),
        )?
        .filter(|(_, loan)| {
            loan.storage_owner == facts::PlaceRoot::Symbol(owner)
                && loan.statement_index == formation as usize
        }) {
            let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) = operation
            else {
                continue;
            };
            let path = trace_query(
                machine,
                state.symbol,
                "receiver-carrier-path",
                carrier_path(program, reference, facts.borrow.loan_owner_path(loan)),
                &(receiver, owner, formation, reference, loan),
            )?;
            let sources = trace_query(
                machine,
                state.symbol,
                "stored-carrier-sources",
                validation::reference_result_custody::stored_carrier_sources(
                    program, facts, *value, &path,
                ),
                &(receiver, owner, formation, value, &path),
            )?;
            for source in sources {
                let validation::reference_result_custody::StoredCarrierSource::Place(source) =
                    source
                else {
                    // Borrowed calls form their own descriptor. Owned carrier
                    // operands retain the producer's supplying occurrence
                    // above; neither route invents historical correspondence.
                    continue;
                };
                let (source_owner, source_formation) = match source.source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        let mut holders = results
                            .iter()
                            .filter(|(binding, _)| binding.binding_ordinal == binding_ordinal);
                        let (binding, facts::PlaceRoot::Symbol(owner)) = trace_query(
                            machine,
                            state.symbol,
                            "supplying-result-owner",
                            holders.next(),
                            &(receiver, binding_ordinal),
                        )?
                        else {
                            return trace_query(
                                machine,
                                state.symbol,
                                "supplying-result-root-kind",
                                None,
                                &(receiver, binding_ordinal),
                            );
                        };
                        if holders.next().is_some() {
                            return trace_query(
                                machine,
                                state.symbol,
                                "duplicate-supplying-result-owner",
                                None,
                                &(receiver, binding_ordinal),
                            );
                        }
                        (*owner, Some(binding.statement_index as usize))
                    }
                    _ => match super::stores::source_owner(program, state, &source.source) {
                        Some(owner) => (owner, None),
                        _ => continue,
                    },
                };
                let Some(supplying_type) =
                    validation::reference_result_custody::storage_owner_type_reference(
                        program,
                        state,
                        source_owner,
                        formation,
                    )
                else {
                    continue;
                };
                let mut supplying = Vec::new();
                for (handle, candidate) in trace_query(
                    machine,
                    state.symbol,
                    "supplying-resource-loans",
                    facts.borrow.statement_resource_loans(borrowing),
                    &(receiver, source_owner, source_formation),
                )?
                .filter(|(_, candidate)| {
                    candidate.storage_owner == facts::PlaceRoot::Symbol(source_owner)
                        && order.precedes(candidate.statement_index, loan.statement_index)
                        && source_formation
                            .is_none_or(|formation| candidate.statement_index == formation)
                }) {
                    if trace_query(
                        machine,
                        state.symbol,
                        "supplying-carrier-path",
                        carrier_path(
                            program,
                            supplying_type,
                            facts.borrow.loan_owner_path(candidate),
                        ),
                        &(receiver, handle, source_owner, supplying_type, candidate),
                    )? == source.path
                    {
                        supplying.push((handle, candidate.statement_index));
                    }
                }
                // Replacements establish a new occurrence at the same slot.
                // Every alternative at that latest formation is retained.
                let Some(formation) = supplying
                    .iter()
                    .map(|(_, statement)| *statement)
                    .max_by_key(|statement| order.position(*statement))
                else {
                    // Source borrow admission already owns this carrier. With
                    // no supplying installed loan there is no local parent to
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

/// Follow complete historical suppliers, including intermediate invocation
/// associations. Only callers select the receiving resource whose death is
/// owed; reachability itself neither disposes a supplier nor grants access.
pub(super) fn supplies(
    transported: &TransportedLoans,
    supplier: arena::Handle<BorrowLoanFact>,
    receiver: arena::Handle<BorrowLoanFact>,
) -> bool {
    let mut pending = vec![receiver];
    let mut visited = Vec::new();
    while let Some(receiver) = pending.pop() {
        if receiver == supplier {
            return true;
        }
        if visited.contains(&receiver) {
            continue;
        }
        visited.push(receiver);
        pending.extend(
            transported
                .iter()
                .filter_map(|(received, supplying)| (*received == receiver).then_some(*supplying)),
        );
    }
    false
}

/// Temporary failure-only imaging of an existing query result. No second
/// semantic lookup is performed; the original Option is returned unchanged.
/// Remove before publication once the retained refusal is explained.
pub(in crate::execution::terminal_unit::control::statement_sequence) fn trace_query<T>(
    machine: SymbolHandle,
    state: SymbolHandle,
    query: &str,
    result: Option<T>,
    occurrence: &impl std::fmt::Debug,
) -> Option<T> {
    if result.is_none() && std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some() {
        eprintln!(
            "REFERENCE_TRANSPORT_REFUSAL machine={machine:?} state={state:?} query={query} occurrence={occurrence:?}"
        );
    }
    result
}
