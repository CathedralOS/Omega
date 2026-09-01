use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::{
    BorrowAccessKind, CheckedBorrowResourceDispositionTarget, CheckedBorrowResourceLifecyclePhase,
    CheckedParentBorrowResource, CheckedReborrowAccessEffect, CheckedReborrowContainmentKind,
    CheckedReborrowResourceDisposition, FlowBorrowWeakeningReason, FlowConstraintKind,
};
use psi_core::MachineId;
use psi_terminal::{
    TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember,
};

use crate::{CheckedTrees, LoweredSourceCallOccurrence, LoweringError, unsupported};

use crate::reborrow_root_handoff::{access, boundary, identity, owner_path, place, place_segments};

fn span_contains<T>(span: HandleSpan<T>, handle: Handle<T>) -> bool {
    if span.is_empty() || !handle.is_valid() || span.start().generation() != handle.generation() {
        return false;
    }
    let start = span.start().arena_index();
    let end = start.saturating_add(span.count());
    (start..end).contains(&handle.arena_index())
}

pub(crate) fn retain_selected_reborrow_restored_call_uses(
    checked: &CheckedTrees,
    source_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    source_calls: &[LoweredSourceCallOccurrence],
    terminal_machines: &[psi_terminal::TerminalMachine],
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
        let borrow_call = borrow.calls.get(certificate.borrow_call);
        let call_access = borrow.argument_accesses.get(certificate.call_access);
        let parent_constraint = flow
            .contexts
            .constraint_refs
            .get(certificate.parent_entry_constraint);
        if !flow
            .contexts
            .constraint_refs
            .is_valid(child.parent_suspension.parent_entry_constraint)
        {
            return unsupported(
                "restored reborrow call use formation constraint handle is invalid",
            );
        }
        let formation_parent_constraint = flow
            .contexts
            .constraint_refs
            .get(child.parent_suspension.parent_entry_constraint);
        let receiver_free_target = checked
            .typed
            .machines()
            .iter()
            .find_map(|machine| {
                checked
                    .typed
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == call.target_symbol)
                    .map(|state| (machine, state))
            })
            .is_some_and(|(machine, state)| {
                !checked
                    .typed
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.is_self)
                    && (!call.has_receiver || machine.attached_data_symbol == call.receiver_symbol)
            });

        let exclusive_reactivation = matches!(
            child.access,
            BorrowAccessKind::Mutable | BorrowAccessKind::WriteOnly
        ) && child.access_effect
            == CheckedReborrowAccessEffect::ExclusiveSuspension;
        let sole_shared_freeze = child.access == BorrowAccessKind::Read
            && child.access_effect == CheckedReborrowAccessEffect::SharedFreeze
            && borrow
                .reborrow_loan_resources
                .iter()
                .filter(|(_, candidate)| candidate.parent_loan == parent.loan)
                .count()
                == 1;
        let exact_resource = certificate.machine_symbol == child.machine_symbol
            && certificate.machine_symbol == parent.machine_symbol
            && certificate.state_symbol == child.state_symbol
            && certificate.state_symbol == parent.state_symbol
            && certificate.child_loan == child.loan
            && certificate.parent_loan == parent.loan
            && child.parent_loan == parent.loan
            && child.parent_resource
                == CheckedParentBorrowResource::DirectRoot {
                    resource: certificate.parent_resource,
                }
            && parent.access == BorrowAccessKind::Mutable
            && parent.owner_path.is_empty()
            && (exclusive_reactivation || sole_shared_freeze)
            && child.weakening_reason == FlowBorrowWeakeningReason::LastUseExpired
            && child.loan == child_activation.loan
            && child.loan == child_weakening.loan
            && child.parent_suspension.child_loan == child.loan
            && child.parent_suspension.parent_loan == parent.loan
            && child.parent_suspension.parent_resource == child.parent_resource
            && child.parent_suspension.source == child.activation_source
            && formation_parent_constraint.kind
                == FlowConstraintKind::BorrowLoan { loan: parent.loan }
            && child.parent_end_status.child_weakening == certificate.child_weakening
            && child.parent_end_status.child_loan == child.loan
            && child.parent_end_status.parent_loan == parent.loan
            && child.parent_end_status.parent_resource == child.parent_resource
            && child.parent_end_status.status
                == psi_checked_trees::ParentLexicalStatusAtChildEnd::LivePastChild
            && parent_weakening.loan == parent.loan
            && child.restoration.child_weakening_source == child.weakening_source
            && child.restoration.child_weakening_reason == child.weakening_reason
            && certificate.carrier_place.root_symbol == parent.owner_symbol
            && certificate.carrier_place.segments.is_empty()
            && certificate.restored_place == parent.captured_place
            && certificate.access == BorrowAccessKind::Mutable
            && !borrow.reborrow_loan_resources.iter().any(|(_, candidate)| {
                candidate.parent_resource
                    == CheckedParentBorrowResource::Reborrow {
                        resource: certificate.child_resource,
                    }
            });
        let exact_disposition = disposition.machine_symbol == certificate.machine_symbol
            && disposition.state_symbol == certificate.state_symbol
            && disposition.child_loan == child.loan
            && disposition.child_resource == certificate.child_resource
            && disposition.child_activation == child.parent_suspension.child_activation
            && disposition.child_weakening == certificate.child_weakening
            && disposition.parent_loan == parent.loan
            && disposition.parent_resource == child.parent_resource
            && disposition.boundary_source == child.weakening_source
            && disposition.boundary_phase == CheckedBorrowResourceLifecyclePhase::LastUseExpired
            && disposition.retired_parent_path.is_empty()
            && disposition.final_target
                == CheckedBorrowResourceDispositionTarget::ParentResource(
                    child.parent_resource.clone(),
                )
            && if sole_shared_freeze {
                disposition.shared_cohort.as_slice() == [certificate.child_resource]
                    && disposition.disposition
                        == CheckedReborrowResourceDisposition::RestoreSharedCohort
            } else {
                disposition.shared_cohort.is_empty()
                    && disposition.disposition == CheckedReborrowResourceDisposition::Reactivate
            };
        let exact_containment = containment.machine_symbol == certificate.machine_symbol
            && containment.state_symbol == certificate.state_symbol
            && containment.child_loan == child.loan
            && containment.child_resource == certificate.child_resource
            && containment.parent_loan == parent.loan
            && containment.parent_resource == child.parent_resource
            && containment.parent_access == parent.access
            && containment.child_access == child.access
            && containment.access_effect == child.access_effect
            && containment.child_activation == child.parent_suspension.child_activation
            && containment.parent_entry_constraint
                == child.parent_suspension.parent_entry_constraint
            && containment.formation_source == child.activation_source
            && containment.child_weakening == certificate.child_weakening
            && containment.child_weakening_source == child.weakening_source
            && containment.child_weakening_reason == child.weakening_reason
            && containment.parent_weakening == child.parent_end_status.parent_weakening
            && containment.parent_place == parent.captured_place
            && containment.child_place == child.captured_place
            && child
                .captured_place
                .segments
                .starts_with(&parent.captured_place.segments)
            && containment.projection_remainder
                == child.captured_place.segments[parent.captured_place.segments.len()..]
            && containment.containment
                == if sole_shared_freeze {
                    CheckedReborrowContainmentKind::SharedFreeze
                } else {
                    CheckedReborrowContainmentKind::ExclusiveSuspension
                };
        let exact_call = call.statement_index == borrow_call.statement_index
            && call.call_ordinal == borrow_call.call_ordinal
            && call.target_symbol == borrow_call.target_symbol
            && call.receiver_symbol == borrow_call.receiver_symbol
            && call.has_receiver == borrow_call.has_receiver
            && receiver_free_target
            && call.target_symbol == certificate.target_symbol
            && call.accesses == borrow_call.accesses
            && matches!(
                child.weakening_source,
                psi_checked_trees::FlowInvalidationSource::Statement { statement_index }
                    if statement_index == call.statement_index
            )
            && call.accesses.start() == certificate.call_access
            && call.accesses.len() == 1
            && call_access.root_symbol == parent.owner_symbol
            && borrow.access_segments(call_access).is_empty()
            && call_access.kind == BorrowAccessKind::Read
            && span_contains(call.entry_constraints, certificate.parent_entry_constraint)
            && parent_constraint.kind == FlowConstraintKind::BorrowLoan { loan: parent.loan };
        if !exact_resource || !exact_disposition || !exact_containment || !exact_call {
            return unsupported("restored reborrow call use drifted from its checked replay");
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
            .filter(|operation| operation.id == occurrence.terminal_operation)
            .collect::<Vec<_>>();
        let [operation] = operations.as_slice() else {
            return unsupported(
                "restored reborrow call use does not name one exact Terminal operation",
            );
        };
        let psi_terminal::OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } = &operation.kind
        else {
            return unsupported("restored reborrow call use does not name a Unit call");
        };
        let [argument] = structural_arguments.as_slice() else {
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
        let [callee_parameter] = callee.structural_parameters.as_slice() else {
            return unsupported(
                "restored reborrow call use callee does not have one structural parameter",
            );
        };
        let exact_terminal_call = operation.result == psi_terminal::OperationResult::Unit
            && argument.path.is_empty()
            && argument.access == psi_terminal::StructuralAccess::MutableBorrow
            && claim_transfers.is_empty()
            && caller.structural_parameters.iter().any(|parameter| {
                parameter.place == argument.place
                    && parameter.access == psi_terminal::StructuralAccess::MutableBorrow
            })
            && callee.parameters.is_empty()
            && callee.result == psi_terminal::TerminalMachineResult::Unit
            && callee_parameter.position == 0
            && !callee_parameter.is_self
            && callee_parameter.access == psi_terminal::StructuralAccess::MutableBorrow;
        if !exact_terminal_call {
            return unsupported(
                "restored reborrow call use Terminal operation drifted from checked authority",
            );
        }
        let child_owner_identity = identity(checked, child.owner_symbol)?;
        let child_owner_path = owner_path(checked, &child.owner_path)?;
        let child_place = place(checked, &child.captured_place)?;
        let child_access = access(child.access.clone());
        let child_activation = boundary(checked, child_activation.source)?;
        let child_weakening = boundary(checked, child_weakening.source)?;
        let shared_cohort = sole_shared_freeze
            .then(|| TerminalReborrowSharedCohortMember {
                child_owner_identity: child_owner_identity.clone(),
                child_owner_path: child_owner_path.clone(),
                child_place: child_place.clone(),
                child_access,
                child_activation: child_activation.clone(),
                child_weakening: child_weakening.clone(),
            })
            .into_iter()
            .collect();
        rows.push(TerminalReborrowRestoredCallUse {
            machine: terminal_machine,
            operation: occurrence.terminal_operation,
            restoration_class: if sole_shared_freeze {
                TerminalReborrowRestorationClass::SoleSharedFreezeRestoration
            } else {
                TerminalReborrowRestorationClass::ExclusiveReactivation
            },
            call_boundary: psi_terminal::TerminalBorrowBoundarySource::Call {
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
    borrow: &psi_checked_trees::BorrowFacts,
    certificate: &psi_checked_trees::CheckedReborrowRestoredCallUseCertificate,
) -> Result<Handle<psi_checked_trees::FlowBorrowWeakeningFact>, LoweringError> {
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
