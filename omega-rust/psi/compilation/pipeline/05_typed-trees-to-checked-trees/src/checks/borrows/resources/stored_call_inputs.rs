//! A reference saved by a helper keeps the evaluated input that lent it.
//! Bind the completed stored generation to those original call resources.
//! Their full access and parent lifetime stay intact even when the saved
//! pointer selects a smaller part of the input's data.

use arena::Handle;
use checked_trees::{BorrowLoanLineage, CheckFacts, CheckedCallBorrowLoanSource};

pub(super) fn install(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
    facts.borrow.stored_call_inputs.reset_retain_capacity();
    let mut retained = Vec::new();
    for (_, state) in facts.flow.control.states.iter() {
        let Some(owner) = facts
            .borrow
            .unique_state_borrow(state.machine_symbol, state.state_symbol)
        else {
            continue;
        };
        for (loan, row) in facts.borrow.state_loans(owner) {
            if row.lineage != BorrowLoanLineage::UnretainedDerived
                || !row.established_by_call.is_valid()
            {
                continue;
            }
            match retained_inputs(program, facts, state, loan, row) {
                Ok(inputs) => retained.push((loan, inputs)),
                Err(reason) if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() => {
                    eprintln!(
                        "CHECKSITE stored-call-input-unavailable loan={loan:?} reason={reason}"
                    );
                }
                Err(_) => {}
            }
        }
    }
    'loan: for (loan, inputs) in retained {
        let mut rows = Vec::with_capacity(inputs.len());
        for mut draft in inputs {
            draft.input.capture_path = facts.borrow.call_loan_paths.insert_many(draft.path);
            if let Some(place) = draft.incoming {
                let checked_trees::CheckedRetainedBorrowInputPermission::Incoming {
                    path,
                    owner_path,
                    ..
                } = &mut draft.input.permission
                else {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE stored-call-input-unavailable loan={loan:?} reason=incoming draft lost its entry permission"
                        );
                    }
                    continue 'loan;
                };
                *path = facts
                    .borrow
                    .call_loan_backing_paths
                    .insert_many(place.segments);
                *owner_path = facts
                    .borrow
                    .call_loan_backing_paths
                    .insert_many(place.incoming_owner_path);
            }
            rows.push(draft.input);
        }
        let inputs = facts.borrow.retained_borrow_inputs.insert_many(rows);
        facts
            .borrow
            .stored_call_inputs
            .insert(checked_trees::CheckedStoredCallBorrowInputs { loan, inputs });
    }
}

fn retained_inputs(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    loan: Handle<checked_trees::BorrowLoanFact>,
    row: &checked_trees::BorrowLoanFact,
) -> Result<Vec<InputDraft>, &'static str> {
    let descriptors = validation::completed_storage::loan_call_descriptors(
        program,
        facts,
        state.machine_symbol,
        state.state_symbol,
        loan,
    )?
    .ok_or("the saved reference has no completed call generation")?;
    let mut inputs = Vec::new();
    for descriptor in descriptors {
        let origin = checked_trees::CheckedRetainedBorrowInputOrigin::CompletedStore {
            capture: descriptor.capture,
        };
        let source = match descriptor.source {
            validation::completed_storage::CompletedCallDescriptorSource::Incoming(slot) => {
                append_incoming(
                    program,
                    facts,
                    state,
                    descriptor.call,
                    descriptor.capture,
                    slot,
                    &row.kind,
                    &mut inputs,
                )?;
                continue;
            }
            validation::completed_storage::CompletedCallDescriptorSource::Captured(source) => {
                source
            }
        };
        let (suppliers, source_parents) = super::permission_sources::lending_sources(
            program,
            facts,
            state,
            descriptor.call,
            source.capture,
            &source.projection,
            &row.kind,
        )?;
        // Parent handles below belong to this collector's scratch
        // arena. Compare their complete roster, never republish it.
        for supplier in suppliers {
            let original = match supplier.source {
                CheckedCallBorrowLoanSource::CallAccess { .. } => {
                    if !facts.flow.control.operands.is_valid(supplier.actual) {
                        return Err("the saved reference lost its original evaluated input");
                    }
                    crate::checks::borrows::formation_captures::actual_invocation(
                        program,
                        facts,
                        state,
                        supplier.actual,
                    )?
                    .ok_or("the saved reference has no original invocation tuple")?
                }
                CheckedCallBorrowLoanSource::StoredLoan { .. } => descriptor.call,
            };
            let mut found = false;
            for (resource_handle, resource) in facts.borrow.call_loan_resources.iter() {
                if resource.call != original
                    || resource.parent_lifetime.root != supplier.backing.root
                    || resource.access.direct_reborrow_effect(&row.kind).is_none()
                {
                    continue;
                }
                let same_permission = match (&supplier.source, &resource.source) {
                    (
                        CheckedCallBorrowLoanSource::StoredLoan { loan },
                        CheckedCallBorrowLoanSource::StoredLoan { loan: original },
                    ) => loan == original,
                    (
                        CheckedCallBorrowLoanSource::CallAccess {
                            access,
                            selected,
                            parents,
                        },
                        CheckedCallBorrowLoanSource::CallAccess {
                            access: original,
                            selected: original_selected,
                            parents: original_parents,
                        },
                    ) => {
                        resource.operand == supplier.actual
                            && access == original
                            && selected == original_selected
                            && facts.borrow.call_loan_parents.span(*original_parents)
                                == source_parents.span(*parents)
                    }
                    _ => false,
                };
                if !same_permission {
                    continue;
                }
                found = true;
                let call = facts.flow.control.calls.get(resource.call);
                let parameter_index = call
                    .argument_operands()
                    .handles()
                    .position(|actual| actual == resource.owner_operand)
                    .ok_or("the retained permission is absent from its original argument tuple")?;
                let path = facts
                    .borrow
                    .call_loan_paths
                    .span(resource.capture_path)
                    .ok_or("the retained permission lost its captured leaf path")?
                    .to_vec();
                let input = checked_trees::CheckedRetainedBorrowInput {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    origin,
                    call: resource.call,
                    parameter_index,
                    capture_path: resource.capture_path,
                    source_kind: resource
                        .capture_source
                        .ok_or("the retained permission lost its original receiving operation")?,
                    permission: checked_trees::CheckedRetainedBorrowInputPermission::Borrowed {
                        resource: resource_handle,
                    },
                };
                if !inputs.iter().any(|draft: &InputDraft| draft.input == input) {
                    inputs.push(InputDraft {
                        input,
                        path,
                        incoming: None,
                    });
                }
            }
            if !found {
                return Err("the saved reference's original call resource is unavailable");
            }
        }
    }
    if inputs.is_empty() {
        return Err("the saved reference has no complete input resource roster");
    }
    Ok(inputs)
}

struct InputDraft {
    input: checked_trees::CheckedRetainedBorrowInput,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    incoming: Option<checked_trees::CapturedPlace>,
}

#[allow(clippy::too_many_arguments)]
fn append_incoming(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    call: Handle<checked_trees::FlowCallFact>,
    capture: Handle<checked_trees::FlowCallStoredOutput>,
    slot: validation::CanonicalPlace,
    access: &checked_trees::BorrowAccessKind,
    inputs: &mut Vec<InputDraft>,
) -> Result<(), &'static str> {
    let control = &facts.flow.control;
    let invocation = control.calls.get(call);
    let selected = control.call_stored_outputs.get(capture);
    let parameter_index = invocation
        .argument_operands()
        .handles()
        .position(|actual| actual == selected.actual)
        .ok_or("the retained incoming descriptor lost its receiving argument")?;
    let actual = control.operands.get(selected.actual);
    let receiver = validation::reference_shell(program, actual.type_reference)
        .ok_or("the stored descriptor destination has no outer reference")?;
    let path = control
        .operand_retained_paths
        .span(selected.path)
        .ok_or("the retained incoming destination lost its selected path")?;
    let path = validation::corresponding_place_projection(program, receiver.referee, path)
        .ok_or("the retained incoming destination has no applied member correspondence")?;
    let reference =
        validation::project_type_reference_from_segments(program, receiver.referee, &path)
            .and_then(|reference| validation::reference_shell(program, reference))
            .ok_or("the retained incoming destination is not a reference leaf")?;
    let owner_path = crate::borrow::owner_path_from_place_segments(program, &path);
    let capture_path = crate::borrow::carrier_path(program, actual.type_reference, &owner_path)
        .ok_or("the retained incoming descriptor has no typed receiving path")?;
    let aliases = crate::checks::borrows::completed_backings::incoming_sources(
        program,
        facts,
        state,
        invocation.statement_index,
        invocation.entry_constraints,
        slot,
    )?;
    if aliases.is_empty() {
        return Err("the retained incoming descriptor has no original entry alternatives");
    }
    let owner = facts
        .borrow
        .unique_state_borrow(state.machine_symbol, state.state_symbol)
        .ok_or("the retained incoming descriptor has no unique borrowing state")?;
    for alias in aliases {
        let identity = alias.place.incoming_reference;
        if !facts.borrow.state_owns_incoming_reference(owner, identity) {
            return Err("the retained incoming descriptor has no exact entry identity");
        }
        let incoming = facts.borrow.incoming_references.get(identity);
        let owner_path = facts
            .borrow
            .owner_segments
            .span(incoming.owner_path)
            .ok_or("the retained incoming identity lost its original leaf path")?;
        let original = validation::owned_projection_type_reference(
            program,
            incoming.carrier_type,
            &crate::borrow::owner_place_segments(owner_path),
        )
        .and_then(|reference| validation::reference_shell(program, reference))
        .ok_or("the retained incoming identity is not a reference leaf")?;
        if !validation::storage_origin_types_match(program, original.referee, reference.referee)
            || checked_trees::BorrowAccessKind::from(original.access)
                .direct_reborrow_effect(access)
                .is_none()
        {
            return Err(
                "the retained incoming descriptor cannot supply the recorded type and access",
            );
        }
        let access = match access {
            checked_trees::BorrowAccessKind::Read => language_semantics::ReferenceAccess::Shared,
            checked_trees::BorrowAccessKind::Mutable => {
                language_semantics::ReferenceAccess::Mutable
            }
            checked_trees::BorrowAccessKind::WriteOnly => {
                language_semantics::ReferenceAccess::WriteOnly
            }
        };
        inputs.push(InputDraft {
            input: checked_trees::CheckedRetainedBorrowInput {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                origin: checked_trees::CheckedRetainedBorrowInputOrigin::CompletedStore { capture },
                call,
                parameter_index,
                capture_path: arena::HandleSpan::empty(),
                source_kind: checked_trees::ViewReturnInputSource::Stored,
                permission: checked_trees::CheckedRetainedBorrowInputPermission::Incoming {
                    identity,
                    access,
                    root: alias.place.root,
                    path: arena::HandleSpan::empty(),
                    owner_path: arena::HandleSpan::empty(),
                },
            },
            path: capture_path.clone(),
            incoming: Some(alias.place),
        });
    }
    Ok(())
}
