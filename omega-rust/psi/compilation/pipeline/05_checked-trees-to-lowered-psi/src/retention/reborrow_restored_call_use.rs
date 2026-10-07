use arena::Handle;
use checked_trees::{BorrowAccessKind, CheckedReborrowAccessEffect};
use semantic_vocabulary::MachineId;
use terminal_psi::{
    TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember,
};

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use lowered_psi::LoweredSourceCallOccurrence;

use crate::retention::reborrow_root_handoff::{
    access, boundary, identity, owner_path, place, place_segments,
};

pub(crate) fn retain_selected_reborrow_restored_call_uses(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    source_calls: &[LoweredSourceCallOccurrence],
    terminal_machines: &[terminal_psi::TerminalMachine],
    rows: &mut Vec<TerminalReborrowRestoredCallUse>,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    let flow = &checked.facts.flow;
    for (_, certificate) in borrow.reborrow_restored_call_use_certificates.iter() {
        if certificate.machine_symbol != source_machine {
            continue;
        }
        if !borrow
            .reborrow_loan_resources
            .is_valid(certificate.child_resource)
            || !borrow
                .direct_loan_resources
                .is_valid(certificate.parent_resource)
            || !borrow
                .reborrow_disposition_events
                .is_valid(certificate.disposition)
            || !borrow
                .reborrow_containment_certificates
                .is_valid(certificate.containment)
            || !flow
                .borrow_lifetimes
                .weakenings
                .is_valid(certificate.child_weakening)
            || !flow
                .borrow_lifetimes
                .weakenings
                .is_valid(child_parent_weakening_handle(borrow, certificate)?)
            || !flow.control.calls.is_valid(certificate.call)
            || !borrow.calls.is_valid(certificate.borrow_call)
            || !borrow.argument_accesses.is_valid(certificate.call_access)
            || !flow
                .contexts
                .constraint_refs
                .is_valid(certificate.parent_entry_constraint)
        {
            return unsupported("restored reborrow call use contains an invalid checked handle");
        }

        let child = borrow
            .reborrow_loan_resources
            .get(certificate.child_resource);
        let parent = borrow
            .direct_loan_resources
            .get(certificate.parent_resource);
        let disposition = borrow
            .reborrow_disposition_events
            .get(certificate.disposition);
        let containment = borrow
            .reborrow_containment_certificates
            .get(certificate.containment);
        let child_activation = flow
            .borrow_lifetimes
            .activations
            .get(child.parent_suspension.child_activation);
        let child_weakening = flow
            .borrow_lifetimes
            .weakenings
            .get(certificate.child_weakening);
        let parent_weakening = flow
            .borrow_lifetimes
            .weakenings
            .get(child.parent_end_status.parent_weakening);
        let call = flow.control.calls.get(certificate.call);
        if !flow
            .contexts
            .constraint_refs
            .is_valid(child.parent_suspension.parent_entry_constraint)
        {
            return unsupported(
                "restored reborrow call use formation constraint handle is invalid",
            );
        }
        let shared_freeze_cohort = child.access == BorrowAccessKind::Read
            && child.access_effect == CheckedReborrowAccessEffect::SharedFreeze
            && matches!(disposition.shared_cohort.len(), 1..=3);

        let mut shared_cohort = Vec::new();
        if shared_freeze_cohort {
            for member_handle in &disposition.shared_cohort {
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
                    child_owner_identity: identity(checked, member.owner_symbol)?,
                    child_owner_path: owner_path(checked, &member.owner_path)?,
                    child_place: place(checked, &member.captured_place)?,
                    child_access: access(member.access.clone()),
                    child_activation: boundary(checked, member_activation.source)?,
                    child_weakening: boundary(checked, member_weakening.source)?,
                });
            }
        }

        let occurrences = source_calls
            .iter()
            .filter(|occurrence| {
                occurrence.source_state == certificate.state_symbol
                    && occurrence.statement_index == call.statement_index
                    && occurrence.call_ordinal == call.call_ordinal
                    && occurrence.source_target == call.target_symbol
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
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
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
        let child_owner_path = owner_path(checked, &child.owner_path)?;
        let child_place = place(checked, &child.captured_place)?;
        let child_access = access(child.access.clone());
        let child_activation = boundary(checked, child_activation.source)?;
        let child_weakening = boundary(checked, child_weakening.source)?;
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
                target_identity: identity(checked, call.target_symbol)?,
            },
            call_target_machine: callee_id,
            source_machine_identity: identity(checked, certificate.machine_symbol)?,
            source_state_identity: identity(checked, certificate.state_symbol)?,
            direct_root_owner_identity: identity(checked, parent.owner_symbol)?,
            direct_root_owner_path: owner_path(checked, &parent.owner_path)?,
            direct_root_place: place(checked, &parent.captured_place)?,
            direct_root_activation: boundary(checked, parent.activation_source)?,
            direct_root_weakening: boundary(checked, parent_weakening.source)?,
            direct_root_lifetime_identity: identity(checked, parent.parent_lifetime.root_symbol)?,
            child_owner_identity,
            child_owner_path,
            child_place,
            projection_remainder: place_segments(checked, &containment.projection_remainder)?,
            child_access,
            child_activation,
            formation_boundary: boundary(checked, child.parent_suspension.source)?,
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

fn child_parent_weakening_handle(
    borrow: &checked_trees::BorrowFacts,
    certificate: &checked_trees::CheckedReborrowRestoredCallUseCertificate,
) -> Result<Handle<checked_trees::FlowBorrowWeakeningFact>, LoweringError> {
    if !borrow
        .reborrow_loan_resources
        .is_valid(certificate.child_resource)
    {
        return unsupported("restored reborrow call use child resource handle is invalid");
    }
    Ok(borrow
        .reborrow_loan_resources
        .get(certificate.child_resource)
        .parent_end_status
        .parent_weakening)
}
