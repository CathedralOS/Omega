//! A helper's field view must keep the reference that actually supplied it.
//! The checked normal-return relation identifies the completed source; its
//! borrowing permission, presentation and lifetime remain separate obligations.

use super::{formation_capture, nearest_parents, refusal};
use arena::{Arena, Handle};
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
    let mut suppliers = Vec::new();
    let mut parent_rows = Arena::new();
    if !super::super::permission_sources::collect(
        program,
        facts,
        flow_state,
        call,
        source.operand,
        source_path,
        &access,
        false,
        &mut Vec::new(),
        &mut suppliers,
        &mut parent_rows,
    ) || suppliers.is_empty()
    {
        return refusal("returned projection has incomplete captured source permission");
    }
    let mut selected = false;
    for supplier in suppliers {
        let mut place = supplier.backing;
        place.segments.extend_from_slice(projection);
        if crate::flow::compose_captured_view_segments(&mut place.segments).is_none() {
            return refusal("returned projection has no complete source projection");
        }
        let parents = match supplier.source {
            checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                if !facts
                    .flow
                    .borrow_loan_constraints(captured.constraints)
                    .any(|current| current == loan)
                {
                    return refusal("returned projection source was not available at capture");
                }
                nearest_parents(&facts.borrow, state, loan)?
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
                    for parent in nearest_parents(&facts.borrow, state, *parent)? {
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
                                &facts.borrow,
                                state,
                                *other,
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
                    && candidate.root == place.root
                    && candidate.incoming_reference == place.incoming_reference
                    && facts.borrow.loan_segments(candidate) == place.segments
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
            return refusal("returned projection dropped a captured borrowing parent");
        }
        selected |= row.root == place.root
            && facts.borrow.loan_segments(row) == place.segments
            && match row.lineage {
                BorrowLoanLineage::DirectRoot => parents.is_empty(),
                BorrowLoanLineage::Reborrow { parent_loan } => parents.contains(&parent_loan),
                _ => false,
            };
    }
    Ok(selected)
}
