//! A helper's field view must keep the reference that actually supplied it.
//! The checked normal-return relation identifies the completed source; its
//! borrowing permission, presentation and lifetime remain separate obligations.

use super::super::super::formation_captures::formation_capture;
use super::refusal;
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
                selected |= validate_source(
                    program, facts, state, flow_state, invocation, loan, row, source,
                )?;
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
    loan: Handle<BorrowLoanFact>,
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
    let (suppliers, _parent_rows) = super::super::permission_sources::lending_sources(
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
        let original_backing = supplier.backing;
        let mut place = original_backing.clone();
        place.segments.extend_from_slice(projection);
        let Some(place) = physical_place(place) else {
            return refusal("returned projection has no complete source projection");
        };
        let selected_source = match supplier.source {
            checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                if !facts
                    .flow
                    .borrow_loan_constraints(captured.constraints)
                    .any(|current| current == loan)
                {
                    return refusal("returned projection source was not available at capture");
                }
                checked_trees::CheckedBorrowSourceOccurrence::SelectedLoan { loan }
            }
            checked_trees::CheckedCallBorrowLoanSource::CallAccess {
                access: argument,
                selected,
                ..
            } => {
                let argument = facts.borrow.argument_accesses.get(argument);
                if argument.expression != captured.expression
                    || argument.kind.direct_reborrow_effect(&row.kind).is_none()
                {
                    return refusal("returned projection has no matching explicit argument borrow");
                }
                selected
            }
        };
        let physical = crate::checks::borrows::resolve_physical_borrow_sources(
            program,
            facts,
            state,
            selected_source,
            original_backing,
            &row.kind,
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "returned projection's selected source is unavailable: {reason}"
            ))]
        })?;
        let normal_generations = physical
            .iter()
            .filter_map(|source| match source.selected {
                checked_trees::CheckedBorrowSourceOccurrence::SelectedLoan { loan }
                    if facts.borrow.retained_call_input_span(loan).is_some() =>
                {
                    Some(loan)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let represented = facts
            .borrow
            .state_loans(state)
            .filter(|(_, candidate)| {
                candidate.formation == BorrowLoanFormation::ReturnedProjection
                    && candidate.formation_expression == row.formation_expression
                    && candidate
                        .formation_owner_path
                        .and_then(|path| facts.borrow.owner_segments.span(path))
                        .is_some_and(|path| {
                            Some(path)
                                == row
                                    .formation_owner_path
                                    .and_then(|path| facts.borrow.owner_segments.span(path))
                        })
                    && candidate.statement_index == row.statement_index
                    && candidate.storage_owner == row.storage_owner
                    && candidate.kind == row.kind
                    && facts.borrow.loan_owner_path(candidate) == facts.borrow.loan_owner_path(row)
                    && captured_place(&facts.borrow, candidate).as_ref() == Some(&place)
            })
            .collect::<Vec<_>>();
        for physical in physical {
            let generation = match physical.selected {
                checked_trees::CheckedBorrowSourceOccurrence::SelectedLoan { loan }
                    if facts.borrow.retained_call_input_span(loan).is_some() =>
                {
                    Some(loan)
                }
                _ => None,
            };
            // The physical stored generation and the original borrowing
            // permission are different records. Authenticate the latter before
            // comparing early Reborrow parents; do not manufacture parent=D.
            let permissions = crate::checks::borrows::returned_borrow_permission_sources(
                program,
                facts,
                state,
                physical.selected,
                physical.backing,
                &row.kind,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "returned projection's original permission is unavailable: {reason}"
                ))]
            })?;
            let mut parents = Vec::new();
            let mut direct = false;
            for source in permissions {
                match source.selected {
                    checked_trees::CheckedBorrowSourceOccurrence::DirectStorage
                    | checked_trees::CheckedBorrowSourceOccurrence::FixedIncoming { .. } => {
                        direct = true
                    }
                    checked_trees::CheckedBorrowSourceOccurrence::SelectedLoan { loan } => {
                        crate::shared::collections::push_unique(&mut parents, loan);
                    }
                    checked_trees::CheckedBorrowSourceOccurrence::Unknown => {
                        return refusal(
                            "returned projection has an unavailable original permission",
                        );
                    }
                }
            }
            let complete_permissions = (!direct
                || represented
                    .iter()
                    .any(|(_, candidate)| candidate.lineage == BorrowLoanLineage::DirectRoot))
                && parents.iter().all(|parent| {
                    represented.iter().any(|(_, candidate)| {
                        candidate.lineage
                            == BorrowLoanLineage::Reborrow {
                                parent_loan: *parent,
                            }
                    })
                });
            let transfer = generation.map(|generation| ReturnedTransfer {
                program,
                facts,
                state,
                generation,
                allowed: &normal_generations,
                place: &place,
            });
            let mut complete_transfer = false;
            if let Some(transfer) = &transfer {
                for (candidate, fact) in &represented {
                    complete_transfer |= transfer.matches(*candidate, fact).map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "returned descriptor's recorded transfer is unavailable: {reason}"
                        ))]
                    })?;
                }
            }
            if !complete_permissions && !complete_transfer {
                return refusal(
                    "returned projection dropped a selected source permission or authenticated transfer",
                );
            }
            let owns_permission = match row.lineage {
                BorrowLoanLineage::DirectRoot => direct,
                BorrowLoanLineage::Reborrow { parent_loan } => parents.contains(&parent_loan),
                BorrowLoanLineage::UnretainedDerived => {
                    if let Some(transfer) = &transfer {
                        transfer
                            .matches(loan, row)
                            .map_err(|reason| vec![Diagnostic::error(reason)])?
                    } else {
                        false
                    }
                }
                _ => false,
            };
            selected |=
                captured_place(&facts.borrow, row).as_ref() == Some(&place) && owns_permission;
        }
    }
    Ok(selected)
}

/// A copied returned descriptor may have no new local borrowing resource.
/// Its recorded source must still be the exact normal generation(s) selected
/// by this invocation and projection. This validates that transfer; it neither
/// mints a Reborrow nor widens the source's region or lifetime window.
struct ReturnedTransfer<'a> {
    program: &'a dyn validation::CheckingInput,
    facts: &'a CheckFacts,
    state: &'a checked_trees::StateBorrowFact,
    generation: Handle<BorrowLoanFact>,
    allowed: &'a [Handle<BorrowLoanFact>],
    place: &'a checked_trees::CapturedPlace,
}

impl ReturnedTransfer<'_> {
    fn matches(
        &self,
        handle: Handle<BorrowLoanFact>,
        row: &BorrowLoanFact,
    ) -> Result<bool, &'static str> {
        if row.lineage != BorrowLoanLineage::UnretainedDerived {
            return Ok(false);
        }
        if captured_place(&self.facts.borrow, row).as_ref() != Some(self.place) {
            return Ok(false);
        }
        let sources = crate::checks::borrows::resolve_physical_borrow_sources(
            self.program,
            self.facts,
            self.state,
            checked_trees::CheckedBorrowSourceOccurrence::SelectedLoan { loan: handle },
            self.place.clone(),
            &row.kind,
        )?;
        // The common resolver has authenticated all supplier handles before
        // this narrower source-owner/access predicate dereferences them.
        // A fixed-entry empty terminal is not this recorded transfer role.
        if crate::checks::borrows::transport_suppliers(&self.facts.borrow, self.state, handle)
            .is_none()
        {
            return Ok(false);
        }
        let mut matched = false;
        for source in sources {
            let checked_trees::CheckedBorrowSourceOccurrence::SelectedLoan { loan } =
                source.selected
            else {
                return Ok(false);
            };
            if !self.allowed.contains(&loan)
                || self.facts.borrow.retained_call_input_span(loan).is_none()
            {
                return Ok(false);
            }
            matched |= loan == self.generation;
        }
        Ok(matched)
    }
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
