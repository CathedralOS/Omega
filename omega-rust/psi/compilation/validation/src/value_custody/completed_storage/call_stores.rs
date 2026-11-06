//! Find the reference generation installed by a helper that actually returned.
//! The original call, its published loans and its completed storage must agree.
//! This identifies a stored value; its pointer permission and lifetime remain
//! separate obligations for the consuming operation.

use arena::Handle;
use checked_trees::{BorrowLoanFact, BorrowLoanFormation, CheckFacts, FlowCallFact};
use symbols::SymbolHandle;

pub struct CompletedCallDescriptor {
    pub call: Handle<FlowCallFact>,
    pub loan: Handle<BorrowLoanFact>,
    /// Exact evaluated destination published by this normal call's store.
    pub capture: Handle<checked_trees::FlowCallStoredOutput>,
    pub source: CompletedCallDescriptorSource,
    pub origin: CompletedCallDescriptorOrigin,
}

#[derive(Clone)]
pub enum CompletedCallDescriptorSource {
    Captured(super::StoredCapture),
    Incoming(crate::CanonicalPlace),
}

pub enum CompletedCallDescriptorOrigin {
    Produced,
    Retained,
}

/// Statement continuation can retain several call generations. Resolve every
/// original occurrence, preserving their identities and refusing an incomplete
/// or cyclic supplier roster instead of choosing a convenient same-home call.
pub fn loan_call_descriptors(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    loan: Handle<BorrowLoanFact>,
) -> Result<Option<Vec<CompletedCallDescriptor>>, &'static str> {
    let calls = facts
        .flow
        .control
        .unique_state(machine, state)
        .ok_or("the installed reference has no unique flow state")?
        .calls;
    loan_call_descriptors_at_calls(
        program,
        &facts.borrow,
        &facts.flow.contexts,
        &facts.flow.control,
        machine,
        state,
        calls,
        loan,
    )
}

/// A helper may already have saved a pointer while its caller is still being
/// checked. Use the caller's completed calls without pretending the whole state
/// is finished. Its builder supplies the owning span; every original invocation,
/// tuple, destination and storage-cut check still runs.
#[allow(clippy::too_many_arguments)]
pub fn loan_call_descriptors_at_calls(
    program: &dyn crate::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    contexts: &checked_trees::FlowContextFacts,
    control: &checked_trees::FlowControlFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    calls: arena::HandleSpan<FlowCallFact>,
    loan: Handle<BorrowLoanFact>,
) -> Result<Option<Vec<CompletedCallDescriptor>>, &'static str> {
    let owner = borrow
        .unique_state_borrow(machine, state)
        .ok_or("the installed reference has no unique borrowing state")?;
    if !borrow.state_owns_loan(owner, loan) {
        return Err("the installed reference belongs to another borrowing state");
    }
    if borrow.loans.get(loan).formation != BorrowLoanFormation::ReturnedDescriptor {
        return Ok(None);
    }
    if control.calls.span(calls).is_none() {
        return Err("the installed reference has no complete owning call span");
    }
    let mut pending = vec![(loan, false)];
    let mut active = Vec::new();
    let mut completed = Vec::new();
    let mut output = Vec::new();
    while let Some((handle, closing)) = pending.pop() {
        if closing {
            active.pop();
            completed.push(handle);
            continue;
        }
        if completed.contains(&handle) {
            continue;
        }
        if active.contains(&handle) || !borrow.state_owns_loan(owner, handle) {
            return Err("the installed reference has a cyclic or foreign supplier");
        }
        let row = borrow.loans.get(handle);
        if row.formation != BorrowLoanFormation::ReturnedDescriptor {
            return Err("the installed reference supplier is not a call generation");
        }
        active.push(handle);
        pending.push((handle, true));
        if !row.established_by_call.is_valid() {
            let suppliers = borrow
                .loan_suppliers
                .span(row.supplying_loans)
                .filter(|suppliers| !suppliers.is_empty())
                .ok_or("the installed reference lost its original call generation")?;
            for supplier in suppliers.iter().rev() {
                pending.push((*supplier, false));
            }
            continue;
        }
        if !owner
            .calls
            .handles()
            .any(|call| call == row.established_by_call)
            || !borrow.calls.is_valid(row.established_by_call)
        {
            return Err("the installed reference names another state's call");
        }
        let source = borrow.calls.get(row.established_by_call);
        if source.statement_index != row.statement_index
            || !source
                .completed_reference_loans
                .handles()
                .any(|loan| loan == handle)
        {
            return Err("the installed reference is not in its call's completed roster");
        }
        let mut calls = calls.handles().filter(|call| {
            let call = control.calls.get(*call);
            call.statement_index == source.statement_index
                && call.call_ordinal == source.call_ordinal
                && call.target_symbol == source.target_symbol
                && call.authored_expression == source.authored_expression
                && contexts
                    .borrow_loan_constraints(call.exit_constraints)
                    .any(|loan| loan == handle)
        });
        let call = calls
            .next()
            .ok_or("the installed reference has no performed normal completion")?;
        if calls.next().is_some() {
            return Err("the installed reference has ambiguous normal completions");
        }
        let invocation = control.calls.get(call);
        let root = crate::CanonicalPlace {
            root: row.storage_owner,
            segments: Vec::new(),
        };
        let owner_type =
            crate::canonical_place_type_reference(program, state, row.statement_index, &root)
                .ok_or("the installed descriptor lost its physical owner type")?;
        let destination = crate::CanonicalPlace {
            root: root.root,
            segments: crate::corresponding_place_projection(
                program,
                owner_type,
                &super::loan_homes::place_segments(borrow.loan_owner_path(row)),
            )
            .ok_or("the installed descriptor lost its physical member correspondence")?,
        };
        let mut captures = invocation.stored_outputs.handles().filter(|output| {
            let output = control.call_stored_outputs.get(*output);
            output.destination.root == destination.root
                && control
                    .operand_referent_segments
                    .span(output.destination.segments)
                    .is_some_and(|path| {
                        path.len() == destination.segments.len()
                            && path.iter().zip(&destination.segments).all(|(left, right)| {
                                crate::storage_path_relations::canonical_place_segments_equal(
                                    program, *left, *right,
                                )
                            })
                    })
        });
        let capture = captures
            .next()
            .ok_or("the installed descriptor has no original evaluated destination")?;
        if captures.next().is_some() {
            return Err("the installed descriptor has ambiguous evaluated destinations");
        }
        let selected = control.call_stored_outputs.get(capture);
        if !invocation
            .argument_operands()
            .handles()
            .any(|actual| actual == selected.actual)
            || control.operand_retained_paths.span(selected.path).is_none()
        {
            return Err("the installed descriptor destination is absent from its actual tuple");
        }
        let stored = super::sources_at_snapshot(
            program,
            control,
            invocation.completed_storage,
            &destination,
            super::StoredObservation::Binding,
        )
        .ok_or("the completed call has no complete installed descriptor frontier")?;
        let sources = descriptor_sources(program, stored)?;
        let mut entry = None;
        for source in sources {
            let produced = match &source {
                CompletedCallDescriptorSource::Captured(captured) => {
                    let value = control.operands.get(captured.capture);
                    if value.state_symbol != state {
                        return Err("the installed descriptor belongs to another state");
                    }
                    let reference = crate::project_type_reference_from_segments(
                        program,
                        captured.reference,
                        &captured.projection,
                    )
                    .and_then(|reference| crate::reference_shell(program, reference))
                    .ok_or("the completed store does not contain the declared reference")?;
                    // Debts retain their original input access, independently
                    // of the physical pointer stored beside them.
                    if row.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                        && checked_trees::BorrowAccessKind::from(reference.access)
                            .direct_reborrow_effect(&row.kind)
                            .is_none()
                    {
                        return Err("the installed descriptor cannot supply its recorded access");
                    }
                    value.statement_index == row.statement_index
                        && value.storage_snapshot == invocation.invocation_storage
                }
                CompletedCallDescriptorSource::Incoming(_) => false,
            };
            let origin = if produced {
                CompletedCallDescriptorOrigin::Produced
            } else {
                // A conditional store may keep the old descriptor. Confirm
                // that exact source at entry; it keeps its earlier identity
                // instead of acquiring a new permission from this call.
                if entry.is_none() {
                    let stored = super::sources_at_snapshot(
                        program,
                        control,
                        invocation.invocation_storage,
                        &destination,
                        super::StoredObservation::Binding,
                    )
                    .ok_or("the retained descriptor has no complete call-entry frontier")?;
                    entry = Some(descriptor_sources(program, stored)?);
                }
                if !entry
                    .as_ref()
                    .ok_or("the retained descriptor lost its call-entry frontier")?
                    .iter()
                    .any(|old| same_source(old, &source))
                {
                    return Err(
                        "the retained descriptor is not an original call-entry alternative",
                    );
                }
                CompletedCallDescriptorOrigin::Retained
            };
            output.push(CompletedCallDescriptor {
                call,
                loan: handle,
                capture,
                source,
                origin,
            });
        }
    }
    Ok(Some(output))
}

/// Keep every typed alternative in a completed descriptor frontier. An unknown
/// or empty frontier supplies no source, including beside a valid alternative.
pub fn descriptor_sources(
    program: &dyn crate::CheckingInput,
    stored: super::StoredOperand,
) -> Result<Vec<CompletedCallDescriptorSource>, &'static str> {
    let mut pending = vec![stored];
    let mut output = Vec::new();
    while let Some(source) = pending.pop() {
        match source {
            super::StoredOperand::Unavailable => {
                return Err("the descriptor frontier is unavailable");
            }
            super::StoredOperand::Alternatives(alternatives) => {
                if alternatives.is_empty() {
                    return Err("the descriptor frontier has no alternatives");
                }
                pending.extend(alternatives.into_iter().rev());
            }
            super::StoredOperand::Incoming(place) => {
                output.push(CompletedCallDescriptorSource::Incoming(place));
            }
            super::StoredOperand::Value {
                capture,
                reference,
                projection,
                slot,
            } => {
                let projection =
                    crate::corresponding_place_projection(program, reference, &projection)
                        .ok_or("the stored descriptor lost its applied member correspondence")?;
                output.push(CompletedCallDescriptorSource::Captured(
                    super::StoredCapture {
                        capture,
                        reference,
                        projection,
                        slot,
                    },
                ));
            }
        }
    }
    Ok(output)
}

fn same_source(
    left: &CompletedCallDescriptorSource,
    right: &CompletedCallDescriptorSource,
) -> bool {
    match (left, right) {
        (
            CompletedCallDescriptorSource::Captured(left),
            CompletedCallDescriptorSource::Captured(right),
        ) => left.capture == right.capture && left.projection == right.projection,
        (
            CompletedCallDescriptorSource::Incoming(left),
            CompletedCallDescriptorSource::Incoming(right),
        ) => left == right,
        _ => false,
    }
}

pub fn matches_call_descriptor_frontier(
    program: &dyn crate::CheckingInput,
    stored: super::StoredOperand,
    descriptors: &[CompletedCallDescriptor],
) -> Result<bool, &'static str> {
    let sources = descriptor_sources(program, stored)?;
    Ok(!sources.is_empty()
        && sources.iter().all(|source| {
            descriptors
                .iter()
                .any(|descriptor| same_source(source, &descriptor.source))
        }))
}
