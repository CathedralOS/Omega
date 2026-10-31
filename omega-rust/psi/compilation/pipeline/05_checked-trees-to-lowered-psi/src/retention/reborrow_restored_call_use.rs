use arena::Handle;
use checked_trees::{
    CheckedParentBorrowResource, CheckedReborrowResourceDisposition,
    CheckedReborrowResourceDispositionEvent,
};
use semantic_vocabulary::MachineId;
use terminal_psi::{
    TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember,
};

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use lowered_psi::LoweredSourceCallOccurrence;

mod places;

use places::{Capture, access, boundary, identity, owner_path, place, projected_place};

pub(crate) fn retain_selected_reborrow_restored_call_uses(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    source_calls: &[LoweredSourceCallOccurrence],
    terminal_machines: &[terminal_psi::TerminalMachine],
    rows: &mut Vec<TerminalReborrowRestoredCallUse>,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    let flow = &checked.facts.flow;
    for (_, event) in borrow.reborrow_disposition_events.iter() {
        if event.machine_symbol != source_machine || !event.restored_call.is_valid() {
            continue;
        }
        let shared_freeze_cohort = restored_kind(event)?;
        if !borrow
            .reborrow_loan_resources
            .is_valid(event.child_resource)
            || !matches!(
                event.parent_resource,
                CheckedParentBorrowResource::DirectRoot { resource }
                    if borrow.direct_loan_resources.is_valid(resource)
            )
            || !flow
                .borrow_lifetimes
                .weakenings
                .is_valid(event.child_weakening)
            || !flow
                .borrow_lifetimes
                .weakenings
                .is_valid(child_parent_weakening_handle(borrow, event)?)
            || !flow.control.calls.is_valid(event.restored_call)
            || !borrow.calls.is_valid(event.restored_borrow_call)
            || !borrow
                .argument_accesses
                .is_valid(event.restored_call_access)
        {
            return unsupported("restored reborrow call use contains an invalid checked handle");
        }

        let child = borrow.reborrow_loan_resources.get(event.child_resource);
        let CheckedParentBorrowResource::DirectRoot {
            resource: parent_resource,
        } = event.parent_resource
        else {
            return unsupported("restored reborrow call use names a non-root parent");
        };
        let parent = borrow.direct_loan_resources.get(parent_resource);
        let child_activation = flow
            .borrow_lifetimes
            .activations
            .get(child.parent_suspension.child_activation);
        let child_weakening = flow.borrow_lifetimes.weakenings.get(event.child_weakening);
        let parent_weakening = flow
            .borrow_lifetimes
            .weakenings
            .get(child.parent_end_status.parent_weakening);
        let call = flow.control.calls.get(event.restored_call);
        if !flow
            .contexts
            .constraint_refs
            .is_valid(child.parent_suspension.parent_entry_constraint)
        {
            return unsupported(
                "restored reborrow call use formation constraint handle is invalid",
            );
        }
        if child.machine_symbol != event.machine_symbol
            || child.state_symbol != event.state_symbol
            || child.loan != event.child_loan
            || child.parent_loan != event.parent_loan
            || child.parent_resource != event.parent_resource
            || child.parent_suspension.child_activation != event.child_activation
            || child.parent_end_status.child_weakening != event.child_weakening
            || child.weakening_source != event.boundary_source
            || parent.loan != event.parent_loan
        {
            return unsupported("restored reborrow call use checked rows disagree");
        }

        let direct_root_place = place(
            identities,
            checked,
            &parent.captured_place,
            Capture {
                state: parent.state_symbol,
                loan: parent.loan,
            },
        )?;
        let mut shared_cohort = Vec::new();
        if shared_freeze_cohort {
            for member_handle in &event.shared_cohort {
                let member = borrow.reborrow_loan_resources.get(*member_handle);
                let member_activation = flow
                    .borrow_lifetimes
                    .activations
                    .get(member.parent_suspension.child_activation);
                let member_weakening = flow
                    .borrow_lifetimes
                    .weakenings
                    .get(member.parent_end_status.child_weakening);
                shared_cohort.push(TerminalReborrowSharedCohortMember {
                    child_owner_identity: identities
                        .intern(&identity(checked, member.owner_symbol)?),
                    child_owner_path: owner_path(identities, checked, &member.owner_path)?,
                    child_place: projected_place(
                        identities,
                        checked,
                        &parent.captured_place,
                        &direct_root_place,
                        &member.captured_place,
                        Capture {
                            state: member.state_symbol,
                            loan: member.loan,
                        },
                    )?
                    .0,
                    child_access: access(member.access.clone()),
                    child_activation: boundary(identities, checked, member_activation.source)?,
                    child_weakening: boundary(identities, checked, member_weakening.source)?,
                });
            }
        }

        let occurrences = source_calls
            .iter()
            .filter(|occurrence| {
                occurrence.matches_statement(
                    event.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) && occurrence.source_target == call.target_symbol
            })
            .collect::<Vec<_>>();
        let [occurrence] = occurrences.as_slice() else {
            return unsupported(
                "restored reborrow call use does not name one exact Terminal call occurrence",
            );
        };
        let terminal_operation =
            occurrence
                .terminal_site
                .operation()
                .ok_or(LoweringError::Unsupported(
                    "external completion cannot establish a restored reborrow receipt",
                ))?;
        let caller = terminal_machines
            .iter()
            .find(|machine| machine.id == terminal_machine)
            .ok_or(LoweringError::Unsupported(
                "restored reborrow call use names an absent Terminal machine",
            ))?;
        let operations = caller
            .operations()
            .filter(|operation| operation.id == terminal_operation)
            .collect::<Vec<_>>();
        let [operation] = operations.as_slice() else {
            return unsupported(
                "restored reborrow call use does not name one exact Terminal operation",
            );
        };
        let terminal_psi::OperationKind::CallUnit {
            callee,
            structural_arguments,
            ..
        } = &operation.kind
        else {
            return unsupported("restored reborrow call use does not name a Unit call");
        };
        let [_] = structural_arguments.as_slice() else {
            return unsupported(
                "restored reborrow call use requires one exact structural argument",
            );
        };
        let callee_id = *callee;
        let callee = terminal_machines
            .iter()
            .find(|machine| machine.id == callee_id)
            .ok_or(LoweringError::Unsupported(
                "restored reborrow call use names an absent Terminal callee",
            ))?;
        let [_] = callee.structural_parameters.as_slice() else {
            return unsupported(
                "restored reborrow call use callee does not have one structural parameter",
            );
        };
        let child_owner_identity = identity(checked, child.owner_symbol)?;
        let child_owner_path = owner_path(identities, checked, &child.owner_path)?;
        let (child_place, projection_remainder) = projected_place(
            identities,
            checked,
            &parent.captured_place,
            &direct_root_place,
            &child.captured_place,
            Capture {
                state: child.state_symbol,
                loan: child.loan,
            },
        )?;
        let child_access = access(child.access.clone());
        let child_activation = boundary(identities, checked, child_activation.source)?;
        let child_weakening = boundary(identities, checked, child_weakening.source)?;
        rows.push(TerminalReborrowRestoredCallUse {
            machine: terminal_machine,
            operation: terminal_operation,
            restoration_class: if shared_freeze_cohort {
                TerminalReborrowRestorationClass::SharedFreezeRestoration
            } else {
                TerminalReborrowRestorationClass::ExclusiveReactivation
            },
            call_boundary: terminal_psi::TerminalBorrowBoundarySource::Call {
                statement_index: u64::try_from(call.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "restored reborrow call statement index exceeds Terminal range",
                    )
                })?,
                call_ordinal: u64::try_from(call.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "restored reborrow call ordinal exceeds Terminal range",
                    )
                })?,
                target_identity: identities.intern(&identity(checked, call.target_symbol)?),
            },
            call_target_machine: callee_id,
            source_machine_identity: identities.intern(&identity(checked, event.machine_symbol)?),
            source_state_identity: identities.intern(&identity(checked, event.state_symbol)?),
            direct_root_owner_identity: identities.intern(&identity(checked, parent.owner_symbol)?),
            direct_root_owner_path: owner_path(identities, checked, &parent.owner_path)?,
            direct_root_place,
            direct_root_activation: boundary(identities, checked, parent.activation_source)?,
            direct_root_weakening: boundary(identities, checked, parent_weakening.source)?,
            direct_root_lifetime_identity: identities.intern(&places::storage_root_identity(
                checked,
                parent.parent_lifetime.root,
            )?),
            child_owner_identity: identities.intern(&child_owner_identity),
            child_owner_path,
            child_place,
            projection_remainder,
            child_access,
            child_activation,
            formation_boundary: boundary(identities, checked, child.parent_suspension.source)?,
            child_weakening,
            shared_cohort,
        });
    }
    rows.sort();
    if !rows.windows(2).all(|pair| pair[0] < pair[1]) {
        return unsupported("duplicate restored reborrow call-use publication");
    }
    Ok(())
}

/// The event's disposition kind decides the Terminal restoration class, so the
/// bound row cannot publish a class its recorded disposition did not reach.
fn restored_kind(event: &CheckedReborrowResourceDispositionEvent) -> Result<bool, LoweringError> {
    match event.disposition {
        CheckedReborrowResourceDisposition::Reactivate => Ok(false),
        CheckedReborrowResourceDisposition::RestoreSharedCohort => {
            if !matches!(event.shared_cohort.len(), 1..=3) {
                return unsupported(
                    "restored shared-freeze reborrow cohort is outside the retained bound",
                );
            }
            Ok(true)
        }
        _ => unsupported("restored reborrow call use names a non-restoration disposition"),
    }
}

fn child_parent_weakening_handle(
    borrow: &checked_trees::BorrowFacts,
    event: &CheckedReborrowResourceDispositionEvent,
) -> Result<Handle<checked_trees::FlowBorrowWeakeningFact>, LoweringError> {
    if !borrow
        .reborrow_loan_resources
        .is_valid(event.child_resource)
    {
        return unsupported("restored reborrow call use child resource handle is invalid");
    }
    Ok(borrow
        .reborrow_loan_resources
        .get(event.child_resource)
        .parent_end_status
        .parent_weakening)
}
