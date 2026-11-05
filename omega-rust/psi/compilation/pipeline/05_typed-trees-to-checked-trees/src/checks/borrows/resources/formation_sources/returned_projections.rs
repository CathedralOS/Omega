//! A helper's field view must keep the reference that actually supplied it.
//! The checked normal-return relation identifies the completed source; its
//! borrowing permission, presentation and lifetime remain separate obligations.

use super::super::super::formation_captures::formation_capture;
use super::{nearest_parents, refusal};
use arena::Handle;
use checked_trees::{
    BorrowLoanFact, BorrowLoanFormation, BorrowLoanLineage, CheckFacts, FlowOperandFact,
};
use diagnostics::Diagnostic;

pub(super) fn validate(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::StateBorrowFact,
    flow_state: &checked_trees::FlowStateFact,
    loan: Handle<BorrowLoanFact>,
    row: &BorrowLoanFact,
) -> Result<(), Vec<Diagnostic>> {
    let (activation, _) = super::super::lifecycle::exact_loan_lifecycle_rows(
        &facts.borrow,
        &facts.flow,
        flow_state,
        loan,
    )
    .map_err(|error| vec![error])?;
    let activation = facts.flow.borrow_lifetimes.activations.get(activation);
    let capture = formation_capture(facts, activation.formation_value, row.formation_expression)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "returned view lost its completed invocation capture",
            )]
        })?;
    let value = facts.flow.control.operands.get(capture);
    let path = crate::borrow::owner_place_segments(
        facts.borrow.owner_segments.span_or_empty(
            row.formation_owner_path
                .ok_or_else(|| vec![Diagnostic::error("returned view lost its result path")])?,
        ),
    );
    let profiles = validation::completed_storage::captured_retention_profiles_at_path(
        program,
        &facts.flow.control,
        value,
        &path,
    )
    .ok_or_else(|| {
        vec![Diagnostic::error(
            "returned view has no complete formation frontier",
        )]
    })?;
    let mut pending = profiles
        .into_iter()
        .map(|profile| (profile, false))
        .collect::<Vec<_>>();
    let mut active = Vec::new();
    let mut completed = Vec::new();
    let mut selected = false;
    while let Some((profile, closing)) = pending.pop() {
        if closing {
            active.pop();
            completed.push(profile);
            continue;
        }
        if active.contains(&profile) {
            return refusal("returned view has a cyclic formation alternative");
        }
        if completed.contains(&profile) {
            continue;
        }
        if !facts.flow.control.operand_retentions.is_valid(profile) {
            return refusal("returned view has an unavailable formation alternative");
        }
        active.push(profile);
        pending.push((profile, true));
        let profile = facts.flow.control.operand_retentions.get(profile);
        if facts.flow.control.operands.is_valid(profile.descriptor) {
            let descriptor = facts.flow.control.operands.get(profile.descriptor);
            let checked_trees::FlowOperandCompletion::InvocationResult {
                invocation,
                path: result_path,
                ..
            } = descriptor.completion
            else {
                return refusal("returned projection is not bound to a completed invocation");
            };
            if !facts.flow.control.calls.is_valid(invocation)
                || !flow_state.calls.handles().any(|call| call == invocation)
                || descriptor.state_symbol != state.state_symbol
                || descriptor.statement_index != row.statement_index
                || !facts
                    .flow
                    .control
                    .operand_retentions
                    .is_valid(descriptor.retention)
            {
                return refusal("returned projection has no current invocation identity");
            }
            let call = facts.flow.control.calls.get(invocation);
            if call.authored_expression != row.formation_expression
                || call.statement_index != row.statement_index
                || facts
                    .flow
                    .control
                    .operand_retained_paths
                    .span_or_empty(result_path)
                    != path
            {
                return refusal(
                    "returned projection does not match this invocation and result path",
                );
            }
            let descriptor_profile = facts
                .flow
                .control
                .operand_retentions
                .get(descriptor.retention);
            if !descriptor_profile.alternatives.is_empty() {
                pending.extend(
                    descriptor_profile
                        .alternatives
                        .handles()
                        .map(|profile| (profile, false)),
                );
                continue;
            }
            if descriptor_profile.reborrow_sources.is_empty() {
                return refusal("returned projection lost its actual borrowing source");
            }
            for source in facts
                .flow
                .control
                .operand_reborrow_sources
                .span_or_empty(descriptor_profile.reborrow_sources)
            {
                selected |=
                    validate_source(program, facts, state, flow_state, invocation, row, source)?;
            }
        } else if !profile.alternatives.is_empty() {
            pending.extend(
                profile
                    .alternatives
                    .handles()
                    .map(|profile| (profile, false)),
            );
        } else {
            return refusal("returned projection has no checked descriptor formation");
        }
    }
    if !selected {
        return refusal("returned projection has no matching actual borrowing parent");
    }
    Ok(())
}

fn validate_source(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::StateBorrowFact,
    flow_state: &checked_trees::FlowStateFact,
    call: Handle<checked_trees::FlowCallFact>,
    row: &BorrowLoanFact,
    source: &checked_trees::FlowOperandReborrowSource,
) -> Result<bool, Vec<Diagnostic>> {
    let captured: &FlowOperandFact = facts.flow.control.operands.get(source.operand);
    let source_path = facts
        .flow
        .control
        .operand_retained_paths
        .span_or_empty(source.source_path);
    let projection = facts
        .flow
        .control
        .operand_retained_paths
        .span_or_empty(source.projection);
    let reference = validation::project_type_reference_from_segments(
        program,
        captured.type_reference,
        source_path,
    )
    .and_then(|reference| validation::reference_shell(program, reference))
    .ok_or_else(|| {
        vec![Diagnostic::error(
            "returned projection source is not a captured reference",
        )]
    })?;
    let access = checked_trees::BorrowAccessKind::from(reference.access);
    if projection.is_empty() || access.direct_reborrow_effect(&row.kind).is_none() {
        return refusal("returned projection exceeds its source presentation");
    }
    let (suppliers, parent_rows) = super::super::permission_sources::lending_sources(
        program,
        facts,
        flow_state,
        call,
        source.operand,
        source_path,
        &access,
    )
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "returned projection source permission is unavailable: {reason}"
        ))]
    })?;
    let mut selected = false;
    for supplier in suppliers {
        let mut place = supplier.backing;
        place.segments.extend_from_slice(projection);
        let Some(place) = physical_place(place) else {
            return refusal("returned projection has no complete source projection");
        };
        let parents = match supplier.source {
            checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                if !facts
                    .flow
                    .borrow_loan_constraints(captured.constraints)
                    .any(|current| current == loan)
                {
                    return refusal("returned projection source was not available at capture");
                }
                nearest_parents(program, facts, state, loan)?
            }
            checked_trees::CheckedCallBorrowLoanSource::CallAccess {
                access: argument,
                parents,
            } => {
                let argument = facts.borrow.argument_accesses.get(argument);
                if argument.expression != captured.expression
                    || argument.kind.direct_reborrow_effect(&row.kind).is_none()
                {
                    return refusal("returned projection has no matching explicit argument borrow");
                }
                let mut roots = Vec::new();
                for parent in parent_rows.span_or_empty(parents) {
                    for parent in nearest_parents(program, facts, state, *parent)? {
                        crate::shared::collections::push_unique(&mut roots, parent);
                    }
                }
                // An argument access records its entire ancestry. Its deepest
                // real occurrences lend the result; earlier ancestors do not.
                let mut nearest = Vec::new();
                for candidate in &roots {
                    let ancestor = roots
                        .iter()
                        .filter(|other| *other != candidate)
                        .any(|other| {
                            crate::checks::borrows::aliases::retained_lineage(
                                program, facts, state, *other,
                            )
                            .is_some_and(|lineage| lineage.contains(candidate))
                        });
                    if !ancestor {
                        nearest.push(*candidate);
                    }
                }
                nearest
            }
        };
        let represented = facts
            .borrow
            .state_loans(state)
            .filter(|(_, candidate)| {
                candidate.formation == BorrowLoanFormation::ReturnedProjection
                    && candidate.formation_expression == row.formation_expression
                    && candidate.statement_index == row.statement_index
                    && candidate.storage_owner == row.storage_owner
                    && facts.borrow.loan_owner_path(candidate) == facts.borrow.loan_owner_path(row)
                    && captured_place(&facts.borrow, candidate).as_ref() == Some(&place)
            })
            .collect::<Vec<_>>();
        if parents.is_empty() {
            if !represented
                .iter()
                .any(|(_, candidate)| candidate.lineage == BorrowLoanLineage::DirectRoot)
            {
                return refusal("returned projection dropped its explicit argument borrow");
            }
        } else if !parents.iter().all(|parent| {
            represented.iter().any(|(_, candidate)| {
                candidate.lineage
                    == BorrowLoanLineage::Reborrow {
                        parent_loan: *parent,
                    }
            })
        }) {
            return refusal("returned view did not retain every input borrow it needs");
        }
        selected |= captured_place(&facts.borrow, row).as_ref() == Some(&place)
            && match row.lineage {
                BorrowLoanLineage::DirectRoot => parents.is_empty(),
                BorrowLoanLineage::Reborrow { parent_loan } => parents.contains(&parent_loan),
                _ => false,
            };
    }
    Ok(selected)
}

fn captured_place(
    borrow: &checked_trees::BorrowFacts,
    loan: &BorrowLoanFact,
) -> Option<checked_trees::CapturedPlace> {
    physical_place(checked_trees::CapturedPlace {
        incoming_reference: loan.incoming_reference,
        incoming_owner_path: borrow
            .access_segments
            .span_or_empty(loan.incoming_owner_path)
            .to_vec(),
        root: loan.root,
        segments: borrow.loan_segments(loan).to_vec(),
    })
}

/// Compare footprints in the original collection dimension. Stored loan rows
/// also retain the full input prefix used to suspend outside access; composing
/// a window for this comparison must not rewrite that independent debt.
/// Equal opaque footprints supply no pointer identity or permission: the exact
/// invocation, output path and borrowing-parent checks above still own the join.
fn physical_place(mut place: checked_trees::CapturedPlace) -> Option<checked_trees::CapturedPlace> {
    crate::flow::compose_captured_view_segments(&mut place.segments)?;
    Some(place)
}
