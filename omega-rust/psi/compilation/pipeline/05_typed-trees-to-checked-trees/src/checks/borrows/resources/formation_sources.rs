//! A view formed from a saved reference must use that completed descriptor,
//! including all selected suppliers. Its old storage slot and backing geometry
//! cannot identify or authorize the new view's borrowing parent.

use arena::Handle;
use checked_trees::{BorrowLoanFact, BorrowLoanLineage, CheckFacts, FlowOperandFact};
use diagnostics::Diagnostic;

pub(super) fn validate(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    for (_, state) in facts.borrow.states.iter() {
        let Some(flow_state) = facts
            .flow
            .control
            .state(state.machine_symbol, state.state_symbol)
        else {
            continue;
        };
        for (loan, row) in facts.borrow.state_loans(state) {
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
                flow_state,
                Handle::invalid(),
                capture,
                &[],
                &source_access,
                false,
                &mut Vec::new(),
                &mut selected,
                &mut arena::Arena::new(),
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
                for parent in nearest_parents(&facts.borrow, state, loan)? {
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

fn descriptor_capture(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    source: Handle<FlowOperandFact>,
) -> Option<Handle<FlowOperandFact>> {
    let mut capture = source;
    let mut visited = Vec::new();
    while facts.flow.control.operands.is_valid(capture) && !visited.contains(&capture) {
        visited.push(capture);
        let row = facts.flow.control.operands.get(capture);
        if validation::reference_shell(program, row.type_reference).is_some() {
            return Some(capture);
        }
        row.address_projection?;
        capture = row.backing;
    }
    None
}

fn formation_capture(
    facts: &CheckFacts,
    formation: Handle<FlowOperandFact>,
    source: typed_trees::expression::ExpressionHandle,
) -> Option<Handle<FlowOperandFact>> {
    let mut pending = vec![formation];
    let mut visited = Vec::new();
    let mut selected = Handle::invalid();
    while let Some(capture) = pending.pop() {
        if visited.contains(&capture) || !facts.flow.control.operands.is_valid(capture) {
            continue;
        }
        visited.push(capture);
        let row = facts.flow.control.operands.get(capture);
        if row.expression == source {
            if selected.is_valid() {
                return None;
            }
            selected = capture;
        }
        pending.extend(row.evaluated_inputs.handles());
        pending.push(row.backing);
    }
    selected.is_valid().then_some(selected)
}

fn nearest_parents(
    borrow: &checked_trees::BorrowFacts,
    state: &checked_trees::StateBorrowFact,
    source: Handle<BorrowLoanFact>,
) -> Result<Vec<Handle<BorrowLoanFact>>, Vec<Diagnostic>> {
    if crate::checks::borrows::aliases::retained_lineage(borrow, state, source).is_none() {
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
            BorrowLoanLineage::DirectRoot | BorrowLoanLineage::Reborrow { .. } => {
                parents.push(loan)
            }
            BorrowLoanLineage::UnretainedDerived => {
                let Some(suppliers) =
                    crate::checks::borrows::transport_suppliers(borrow, state, loan)
                else {
                    return refusal("captured descriptor has no authenticated permission transfer");
                };
                pending.extend(suppliers.iter().copied());
            }
            BorrowLoanLineage::LifetimeDependency | BorrowLoanLineage::SuppliedOutput => {
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
