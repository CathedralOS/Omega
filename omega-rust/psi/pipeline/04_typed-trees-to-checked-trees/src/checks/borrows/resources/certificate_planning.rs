//! Planning resource installation, containment certificates and restored
//! call uses from shared cohort observations.

use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowContainmentCertificateDraft, CheckedReborrowDispositionEventDraft,
    CheckedReborrowLoanResourceDraft, CheckedReborrowRestoredCallUseCertificateDraft,
    DispositionTargetIndex, ParentResourceIndex,
};
use crate::checks::borrows::resources::resource_reconstruction::span_handle;
use crate::checks::borrows::resources::retained_validation::{
    reborrow_containment_drift, reborrow_resource_drift, reborrow_restored_call_use_drift,
};
use checked_trees::{
    BorrowFacts, CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
    CheckedReborrowAccessEffect, CheckedReborrowContainmentKind,
    CheckedReborrowResourceDisposition, FlowFacts, FlowInvalidationSource,
    ParentLexicalStatusAtChildEnd,
};
use diagnostics::Diagnostic;

/// Resolve the entire parent graph before either retained arena is reset.
/// Installation is therefore a purely indexed, infallible rewrite.
pub(crate) fn plan_resource_installation(
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
) -> Result<Vec<ParentResourceIndex>, Vec<Diagnostic>> {
    let mut plan = Vec::with_capacity(reborrows.len());
    for (child_index, child) in reborrows.iter().enumerate() {
        let direct_matches = direct
            .iter()
            .enumerate()
            .filter(|(_, resource)| resource.loan == child.parent_loan)
            .map(|(index, _)| ParentResourceIndex::Direct(index));
        let reborrow_matches = reborrows[..child_index]
            .iter()
            .enumerate()
            .filter(|(_, resource)| resource.loan == child.parent_loan)
            .map(|(index, _)| ParentResourceIndex::Reborrow(index));
        let mut matches = direct_matches.chain(reborrow_matches);
        let Some(parent) = matches.next() else {
            return Err(reborrow_resource_drift());
        };
        if matches.next().is_some() {
            return Err(reborrow_resource_drift());
        }
        plan.push(parent);
    }
    Ok(plan)
}

/// Reconstruct the checked-only interval evidence after the complete lifecycle
/// planner has accepted the resource graph. Read/read children never suspend
/// or freeze their parent and therefore have no row.
pub(crate) fn plan_reborrow_containment_certificates(
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
) -> Result<Vec<CheckedReborrowContainmentCertificateDraft>, Vec<Diagnostic>> {
    if installation.len() != reborrows.len() {
        return Err(reborrow_containment_drift());
    }
    let mut certificates = Vec::new();
    for (child_index, child) in reborrows.iter().enumerate() {
        let parent_resource = installation[child_index];
        let (parent_loan, parent_machine, parent_state, parent_access, parent_place) =
            match parent_resource {
                ParentResourceIndex::Direct(index) => {
                    let Some(parent) = direct.get(index) else {
                        return Err(reborrow_containment_drift());
                    };
                    (
                        parent.loan,
                        parent.machine_symbol,
                        parent.state_symbol,
                        &parent.access,
                        &parent.captured_place,
                    )
                }
                ParentResourceIndex::Reborrow(index) => {
                    let Some(parent) = reborrows.get(index) else {
                        return Err(reborrow_containment_drift());
                    };
                    (
                        parent.loan,
                        parent.machine_symbol,
                        parent.state_symbol,
                        &parent.access,
                        &parent.captured_place,
                    )
                }
            };
        if parent_loan != child.parent_loan
            || parent_machine != child.machine_symbol
            || parent_state != child.state_symbol
            || parent_access != &child.parent_access
            || parent_access.direct_reborrow_effect(&child.access) != Some(child.access_effect)
            || parent_place.root_symbol != child.captured_place.root_symbol
            || !child
                .captured_place
                .segments
                .starts_with(&parent_place.segments)
        {
            return Err(reborrow_containment_drift());
        }
        let containment = match child.access_effect {
            CheckedReborrowAccessEffect::SharedRelease => continue,
            CheckedReborrowAccessEffect::SharedFreeze => {
                CheckedReborrowContainmentKind::SharedFreeze
            }
            CheckedReborrowAccessEffect::ExclusiveSuspension => {
                CheckedReborrowContainmentKind::ExclusiveSuspension
            }
        };
        certificates.push(CheckedReborrowContainmentCertificateDraft {
            machine_symbol: child.machine_symbol,
            state_symbol: child.state_symbol,
            child_loan: child.loan,
            child_resource: child_index,
            parent_loan: child.parent_loan,
            parent_resource,
            parent_access: child.parent_access.clone(),
            child_access: child.access.clone(),
            access_effect: child.access_effect,
            child_activation: child.child_activation,
            parent_entry_constraint: child.parent_entry_constraint,
            formation_source: child.activation_source,
            child_weakening: child.child_weakening,
            parent_weakening: child.parent_weakening,
            child_weakening_source: child.weakening_source,
            child_weakening_reason: child.weakening_reason,
            parent_place: parent_place.clone(),
            child_place: child.captured_place.clone(),
            projection_remainder: child.captured_place.segments[parent_place.segments.len()..]
                .to_vec(),
            containment,
        });
    }
    Ok(certificates)
}

fn exact_shared_cohort_observation(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    flow_state: &checked_trees::FlowStateFact,
    borrow_state: &checked_trees::StateBorrowFact,
    cohort: &[(usize, &CheckedReborrowLoanResourceDraft)],
    mutation_statement_index: usize,
) -> bool {
    if mutation_statement_index != cohort.len() + 2
        || !cohort.iter().enumerate().all(|(offset, (_, member))| {
            member.activation_source
                == FlowInvalidationSource::Statement {
                    statement_index: offset + 1,
                }
        })
    {
        return false;
    }
    let Some(observation_statement_index) = mutation_statement_index.checked_sub(1) else {
        return false;
    };
    let observation_calls = flow
        .control
        .calls
        .span_or_empty(flow_state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == observation_statement_index && call.call_ordinal == 0
        })
        .collect::<Vec<_>>();
    let [observation] = observation_calls.as_slice() else {
        return false;
    };
    let Some(target_state) = crate::semantic::calls::find_state(program, observation.target_symbol)
    else {
        return false;
    };
    let Some(target_machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_state.symbol)
    }) else {
        return false;
    };
    if target_machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !program
            .statement_table
            .statements(target_state.statement_nodes)
            .is_empty()
        || program.state_parameters(target_state).len() != cohort.len()
        || program
            .state_parameters(target_state)
            .iter()
            .any(|parameter| {
                parameter.is_self
                    || !matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        typed_trees::types::TypeReferenceNode::Reference {
                            access: language_semantics::ReferenceAccess::Shared,
                            ..
                        }
                    )
            })
        || (observation.has_receiver
            && target_machine.attached_data_symbol != observation.receiver_symbol)
    {
        return false;
    }
    let matching_borrow_calls = borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .filter(|borrow_call| {
            borrow_call.statement_index == observation.statement_index
                && borrow_call.call_ordinal == observation.call_ordinal
                && borrow_call.target_symbol == observation.target_symbol
                && borrow_call.receiver_symbol == observation.receiver_symbol
                && borrow_call.has_receiver == observation.has_receiver
                && borrow_call.accesses == observation.accesses
        })
        .collect::<Vec<_>>();
    let [borrow_call] = matching_borrow_calls.as_slice() else {
        return false;
    };
    let accesses = borrow.argument_accesses.span_or_empty(borrow_call.accesses);
    matches!(cohort.len(), 2 | 3)
        && accesses.len() == cohort.len()
        && accesses.iter().zip(cohort).all(|(access, (_, member))| {
            access.root_symbol == member.owner_symbol
                && borrow.access_segments(access).is_empty()
                && access.kind == checked_trees::BorrowAccessKind::Read
        })
}

/// Retain the first deliberately narrow post-restoration use shapes: one direct
/// exclusive child, or every member of one complete shared-freeze cohort of at
/// most three children, ends by last use immediately before one receiver-free
/// call mutates the whole restored mutable parent carrier. Earlier fully ended
/// sequential exclusive siblings do not invalidate that exact per-child event;
/// the shared form requires the restoration event's exact complete roster.
/// Unsupported shapes remain unclassified rather than receiving inferred
/// authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_reborrow_restored_call_uses(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
    dispositions: &[CheckedReborrowDispositionEventDraft],
    containments: &[CheckedReborrowContainmentCertificateDraft],
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<Vec<CheckedReborrowRestoredCallUseCertificateDraft>, Vec<Diagnostic>> {
    if installation.len() != reborrows.len() {
        return Err(reborrow_restored_call_use_drift());
    }

    let mut certificates = Vec::new();
    for (child_index, child) in reborrows.iter().enumerate() {
        let ParentResourceIndex::Direct(parent_index) = installation[child_index] else {
            continue;
        };
        let Some(parent) = direct.get(parent_index) else {
            return Err(reborrow_restored_call_use_drift());
        };
        let exclusive_reactivation = matches!(
            child.access,
            checked_trees::BorrowAccessKind::Mutable | checked_trees::BorrowAccessKind::WriteOnly
        ) && child.access_effect
            == CheckedReborrowAccessEffect::ExclusiveSuspension;
        let shared_cohort = reborrows
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.parent_loan == parent.loan)
            .collect::<Vec<_>>();
        let bounded_shared_freeze = matches!(shared_cohort.len(), 1..=3)
            && shared_cohort.iter().all(|(_, member)| {
                member.access == checked_trees::BorrowAccessKind::Read
                    && member.access_effect == CheckedReborrowAccessEffect::SharedFreeze
                    && member.machine_symbol == child.machine_symbol
                    && member.state_symbol == child.state_symbol
                    && member.parent_lexical_status == ParentLexicalStatusAtChildEnd::LivePastChild
                    && member.weakening_reason
                        == checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                    && member.weakening_source == child.weakening_source
                    && !reborrows
                        .iter()
                        .any(|candidate| candidate.parent_loan == member.loan)
            });
        if parent.loan != child.parent_loan
            || parent.machine_symbol != child.machine_symbol
            || parent.state_symbol != child.state_symbol
            || parent.access != checked_trees::BorrowAccessKind::Mutable
            || !parent.owner_path.is_empty()
            || (!exclusive_reactivation && !bounded_shared_freeze)
            || child.parent_lexical_status != ParentLexicalStatusAtChildEnd::LivePastChild
            || child.weakening_reason != checked_trees::FlowBorrowWeakeningReason::LastUseExpired
            || reborrows
                .iter()
                .any(|candidate| candidate.parent_loan == child.loan)
        {
            continue;
        }
        let FlowInvalidationSource::Statement { statement_index } = child.weakening_source else {
            continue;
        };

        let matching_dispositions = dispositions
            .iter()
            .enumerate()
            .filter(|(_, disposition)| {
                disposition.child_resource == child_index
                    && disposition.child_loan == child.loan
                    && disposition.parent_loan == parent.loan
                    && disposition.parent_resource == ParentResourceIndex::Direct(parent_index)
                    && disposition.boundary_source == child.weakening_source
                    && disposition.boundary_phase
                        == CheckedBorrowResourceLifecyclePhase::LastUseExpired
                    && disposition.retired_parent_path.is_empty()
                    && disposition.final_target
                        == DispositionTargetIndex::ParentResource(ParentResourceIndex::Direct(
                            parent_index,
                        ))
                    && if bounded_shared_freeze {
                        disposition.shared_cohort
                            == shared_cohort
                                .iter()
                                .map(|(index, _)| *index)
                                .collect::<Vec<_>>()
                            && disposition.disposition
                                == CheckedReborrowResourceDisposition::RestoreSharedCohort
                    } else {
                        disposition.shared_cohort.is_empty()
                            && disposition.disposition
                                == CheckedReborrowResourceDisposition::Reactivate
                    }
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [disposition] = matching_dispositions.as_slice() else {
            continue;
        };
        let matching_containments = containments
            .iter()
            .enumerate()
            .filter(|(_, containment)| {
                containment.child_resource == child_index
                    && containment.child_loan == child.loan
                    && containment.parent_loan == parent.loan
                    && containment.parent_resource == ParentResourceIndex::Direct(parent_index)
                    && if bounded_shared_freeze {
                        containment.access_effect == CheckedReborrowAccessEffect::SharedFreeze
                            && containment.containment
                                == CheckedReborrowContainmentKind::SharedFreeze
                    } else {
                        containment.access_effect
                            == CheckedReborrowAccessEffect::ExclusiveSuspension
                            && containment.containment
                                == CheckedReborrowContainmentKind::ExclusiveSuspension
                    }
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [containment] = matching_containments.as_slice() else {
            continue;
        };

        let matching_flow_states = flow
            .control
            .states
            .iter()
            .filter(|(_, state)| {
                state.machine_symbol == child.machine_symbol
                    && state.state_symbol == child.state_symbol
            })
            .map(|(_, state)| state)
            .collect::<Vec<_>>();
        let [flow_state] = matching_flow_states.as_slice() else {
            continue;
        };
        let calls = flow
            .control
            .calls
            .span_or_empty(flow_state.calls)
            .iter()
            .enumerate()
            .filter(|(_, call)| call.statement_index == statement_index)
            .filter_map(|(offset, call)| {
                span_handle(flow_state.calls, offset).map(|handle| (handle, call))
            })
            .collect::<Vec<_>>();
        let [(call_handle, call)] = calls.as_slice() else {
            continue;
        };
        let Some(target_state) = crate::semantic::calls::find_state(program, call.target_symbol)
        else {
            continue;
        };
        let Some(target_machine) = program.machines().iter().find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == target_state.symbol)
        }) else {
            continue;
        };
        // A nominal type qualifier is source lookup, not a runtime receiver.
        // Keep true instance-receiver calls outside this certificate.
        if call.has_receiver
            && (target_machine.attached_data_symbol != call.receiver_symbol
                || program
                    .state_parameters(target_state)
                    .iter()
                    .any(|parameter| parameter.is_self))
        {
            continue;
        }

        let matching_borrow_states = borrow
            .states
            .iter()
            .filter(|(_, state)| {
                state.machine_symbol == child.machine_symbol
                    && state.state_symbol == child.state_symbol
            })
            .map(|(_, state)| state)
            .collect::<Vec<_>>();
        let [borrow_state] = matching_borrow_states.as_slice() else {
            continue;
        };
        if bounded_shared_freeze
            && shared_cohort.len() > 1
            && !exact_shared_cohort_observation(
                program,
                borrow,
                flow,
                flow_state,
                borrow_state,
                &shared_cohort,
                statement_index,
            )
        {
            continue;
        }
        let borrow_calls = borrow
            .calls
            .span_or_empty(borrow_state.calls)
            .iter()
            .enumerate()
            .filter(|(_, borrow_call)| {
                borrow_call.statement_index == call.statement_index
                    && borrow_call.call_ordinal == call.call_ordinal
                    && borrow_call.target_symbol == call.target_symbol
                    && borrow_call.receiver_symbol == call.receiver_symbol
                    && borrow_call.has_receiver == call.has_receiver
                    && borrow_call.accesses == call.accesses
            })
            .filter_map(|(offset, borrow_call)| {
                span_handle(borrow_state.calls, offset).map(|handle| (handle, borrow_call))
            })
            .collect::<Vec<_>>();
        let [(borrow_call_handle, borrow_call)] = borrow_calls.as_slice() else {
            continue;
        };

        let parameters = program
            .state_parameters(target_state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .collect::<Vec<_>>();
        let [parameter] = parameters.as_slice() else {
            continue;
        };
        if !parameter.is_mutable
            || program
                .state_parameters(target_state)
                .iter()
                .any(|parameter| parameter.is_self)
        {
            continue;
        }
        let typed_trees::types::TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Mutable,
            ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            continue;
        };

        let accesses = borrow.argument_accesses.span_or_empty(call.accesses);
        let [access] = accesses else {
            continue;
        };
        let Some(access_handle) = span_handle(call.accesses, 0) else {
            continue;
        };
        if access.root_symbol != parent.owner_symbol
            || !borrow.access_segments(access).is_empty()
            || access.kind != checked_trees::BorrowAccessKind::Read
        {
            continue;
        }

        let constraints = flow
            .contexts
            .constraint_refs
            .span_or_empty(call.entry_constraints);
        let borrow_call_constraints = constraints
            .iter()
            .filter(|constraint| {
                matches!(
                    constraint.kind,
                    checked_trees::FlowConstraintKind::BorrowCall { .. }
                )
            })
            .collect::<Vec<_>>();
        let access_constraints = constraints
            .iter()
            .filter(|constraint| {
                matches!(
                    constraint.kind,
                    checked_trees::FlowConstraintKind::BorrowAccess { .. }
                )
            })
            .collect::<Vec<_>>();
        let parent_constraints = constraints
            .iter()
            .enumerate()
            .filter(|(_, constraint)| {
                constraint.kind
                    == checked_trees::FlowConstraintKind::BorrowLoan { loan: parent.loan }
            })
            .filter_map(|(offset, _)| span_handle(call.entry_constraints, offset))
            .collect::<Vec<_>>();
        let child_constraint_count = constraints
            .iter()
            .filter(|constraint| {
                shared_cohort.iter().any(|(_, member)| {
                    constraint.kind
                        == checked_trees::FlowConstraintKind::BorrowLoan { loan: member.loan }
                })
            })
            .count();
        let [borrow_call_constraint] = borrow_call_constraints.as_slice() else {
            continue;
        };
        let [access_constraint] = access_constraints.as_slice() else {
            continue;
        };
        let [parent_entry_constraint] = parent_constraints.as_slice() else {
            continue;
        };
        if borrow_call_constraint.kind
            != (checked_trees::FlowConstraintKind::BorrowCall {
                call: *borrow_call_handle,
            })
            || access_constraint.kind
                != (checked_trees::FlowConstraintKind::BorrowAccess {
                    access: access_handle,
                })
            || child_constraint_count != 0
        {
            continue;
        }

        let mutated_places = crate::flow::call_write_accesses(
            program,
            child.machine_symbol,
            child.state_symbol,
            borrow,
            borrow_call,
            mutation_summaries,
        );
        let [mutated_place] = mutated_places.as_slice() else {
            continue;
        };
        if mutated_place.root != facts::PlaceRoot::Symbol(parent.owner_symbol)
            || !mutated_place.segments.is_empty()
        {
            continue;
        }

        certificates.push(CheckedReborrowRestoredCallUseCertificateDraft {
            machine_symbol: child.machine_symbol,
            state_symbol: child.state_symbol,
            child_loan: child.loan,
            child_resource: child_index,
            parent_loan: parent.loan,
            parent_resource: parent_index,
            disposition: *disposition,
            containment: *containment,
            child_weakening: child.child_weakening,
            call: *call_handle,
            borrow_call: *borrow_call_handle,
            call_access: access_handle,
            parent_entry_constraint: *parent_entry_constraint,
            carrier_place: checked_trees::CapturedPlace {
                root_symbol: parent.owner_symbol,
                segments: Vec::new(),
            },
            restored_place: parent.captured_place.clone(),
            access: parent.access.clone(),
            target_symbol: call.target_symbol,
        });
    }
    Ok(certificates)
}
