//! When a new view is made from a copied reference, check the reference that
//! was actually read. Its old slot may now contain a different reference.
//! Every selected supplier must retain its established borrowing parent and
//! presented access; neither the old slot nor equal backing authorizes a view.

use crate::checks::borrows::loan_occurrences::LoanOccurrences;
mod returned_projections;

use super::super::formation_captures::{descriptor_capture, formation_capture};

use arena::Handle;
use checked_trees::{BorrowLoanFact, BorrowLoanLineage, CheckFacts};
use diagnostics::Diagnostic;

pub(super) fn validate(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let prepared_loans = LoanOccurrences::new(program, facts);
    let loan_occurrences = &prepared_loans;
    for (_, state) in facts.borrow.states.iter() {
        let Some(flow_state) = facts
            .flow
            .control
            .state(state.machine_symbol, state.state_symbol)
        else {
            continue;
        };
        for (loan, row) in facts.borrow.state_loans(state) {
            if row.formation == checked_trees::BorrowLoanFormation::ReturnedProjection {
                returned_projections::validate(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    flow_state,
                    loan,
                    row,
                )?;
                continue;
            }
            let BorrowLoanLineage::Reborrow { parent_loan } = row.lineage else {
                continue;
            };
            let suppliers = facts
                .borrow
                .loan_suppliers
                .span_or_empty(row.supplying_loans);
            let [source_loan] = suppliers else {
                if suppliers.is_empty() {
                    continue;
                }
                return refusal("view formation has no exact source descriptor occurrence");
            };
            let (activation, _) = super::lifecycle::exact_loan_lifecycle_rows(
                &facts.borrow,
                &facts.flow,
                flow_state,
                loan,
            )
            .map_err(|error| vec![error])?;
            let activation = facts.flow.borrow_lifetimes.activations.get(activation);
            let formation =
                validation::reference_formation_source(program, row.formation_expression)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "forwarded borrow has no checked view formation",
                        )]
                    })?;
            let capture = formation_capture(facts, activation.formation_value, formation.source)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "view formation lost its exact evaluated source",
                    )]
                })?;
            // A field or indexed place carries its evaluated address receiver.
            // Follow that typed projection to the descriptor lending access;
            // selector operands and equal backing footprints supply none.
            let capture = descriptor_capture(program, facts, capture).ok_or_else(|| {
                vec![Diagnostic::error(
                    "view source lost its captured reference boundary",
                )]
            })?;
            let captured = facts.flow.control.operands.get(capture);
            let Some(source_type) = validation::reference_shell(program, captured.type_reference)
            else {
                return refusal("forwarded view source is not a captured reference descriptor");
            };
            let source_access = checked_trees::BorrowAccessKind::from(source_type.access);
            if source_access.direct_reborrow_effect(&row.kind).is_none() {
                return refusal("view formation exceeds the saved descriptor's presented access");
            }
            if !facts
                .flow
                .borrow_loan_constraints(captured.constraints)
                .any(|loan| loan == *source_loan)
            {
                return refusal("view formation's source descriptor is not current at capture");
            }
            let mut selected = Vec::new();
            if !super::permission_sources::collect(
                program,
                facts,
                loan_occurrences,
                flow_state,
                Handle::invalid(),
                capture,
                &[],
                &source_access,
                false,
                &mut Vec::new(),
                &mut selected,
                &mut arena::Arena::new(),
                &mut super::permission_sources::PermissionFailures::default(),
            ) {
                return refusal(
                    "view formation has incomplete permission suppliers at its captured source",
                );
            }
            let mut parents = Vec::new();
            for source in selected {
                let checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } = source.source
                else {
                    return refusal("view formation requires an established borrowing occurrence");
                };
                for parent in nearest_parents(facts, loan_occurrences, state, loan)? {
                    crate::shared::collections::push_unique(&mut parents, parent);
                }
            }
            if !parents.contains(&parent_loan)
                || !parents.iter().all(|parent| {
                    facts.borrow.state_loans(state).any(|(_, candidate)| {
                        candidate.formation_expression == row.formation_expression
                            && candidate.statement_index == row.statement_index
                            && candidate.storage_owner == row.storage_owner
                            && facts.borrow.loan_owner_path(candidate)
                                == facts.borrow.loan_owner_path(row)
                            && candidate.lineage
                                == (BorrowLoanLineage::Reborrow {
                                    parent_loan: *parent,
                                })
                    })
                })
            {
                return refusal("view formation does not retain every captured borrowing parent");
            }
        }
    }
    Ok(())
}

fn nearest_parents(
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &checked_trees::StateBorrowFact,
    source: Handle<BorrowLoanFact>,
) -> Result<Vec<Handle<BorrowLoanFact>>, Vec<Diagnostic>> {
    let borrow = &facts.borrow;
    if crate::checks::borrows::aliases::retained_lineage(
        facts.view(),
        loan_occurrences,
        state,
        source,
    )
    .is_none()
    {
        return refusal("captured descriptor has incomplete or cyclic borrowing ancestry");
    }
    let mut pending = vec![source];
    let mut visited = Vec::new();
    let mut parents = Vec::new();
    while let Some(loan) = pending.pop() {
        if visited.contains(&loan) {
            continue;
        }
        if !borrow.state_owns_loan(state, loan) {
            return refusal("captured descriptor supplier has no owning borrowing state");
        }
        visited.push(loan);
        match borrow.loans.get(loan).lineage {
            BorrowLoanLineage::DirectRoot
            | BorrowLoanLineage::Reborrow { .. }
            | BorrowLoanLineage::SuppliedOutput => parents.push(loan),
            BorrowLoanLineage::UnretainedDerived => {
                let Some(suppliers) =
                    crate::checks::borrows::transport_suppliers(borrow, state, loan)
                else {
                    return refusal("captured descriptor has no authenticated permission transfer");
                };
                pending.extend(suppliers.iter().copied());
            }
            BorrowLoanLineage::LifetimeDependency => {
                return refusal("captured descriptor has no local borrowing resource");
            }
        }
    }
    if parents.is_empty() {
        return refusal("captured descriptor has no complete borrowing ancestry");
    }
    Ok(parents)
}

fn refusal<T>(message: &str) -> Result<T, Vec<Diagnostic>> {
    Err(vec![Diagnostic::error(message)])
}
