//! Planning and resolving reborrow disposition events.

use crate::checks::borrows::resources::lifecycle::span_handle;
use crate::checks::borrows::resources::lifecycle::{
    DispositionUpdate, EphemeralResourceStatus, EphemeralStatuses, LifecycleBoundaryKey,
    LifecycleEvent, LifecycleEventKind, LifecyclePhase, activation_boundary,
    apply_activation_batch, apply_weakening_batch, exact_resource_lifecycle_handles,
    weakening_event_boundary,
};
use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowDispositionEventDraft, CheckedReborrowLoanResourceDraft, DispositionTargetIndex,
    ParentResourceIndex,
};
use crate::checks::borrows::resources::{
    reborrow_disposition_drift, reborrow_restored_call_use_drift,
};
use checked_trees::{
    BorrowFacts, CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
    CheckedReborrowAccessEffect, CheckedReborrowResourceDisposition, FlowFacts,
    FlowInvalidationSource, ParentLexicalStatusAtChildEnd,
};
use diagnostics::Diagnostic;

pub(crate) fn plan_reborrow_disposition_events(
    flow: &FlowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
) -> Result<Vec<CheckedReborrowDispositionEventDraft>, Vec<Diagnostic>> {
    let mut states = Vec::new();
    for (machine, state) in direct
        .iter()
        .map(|resource| (resource.machine_symbol, resource.state_symbol))
        .chain(
            reborrows
                .iter()
                .map(|resource| (resource.machine_symbol, resource.state_symbol)),
        )
    {
        if !states.contains(&(machine, state)) {
            states.push((machine, state));
        }
    }

    let mut statuses = EphemeralStatuses::new(direct.len(), reborrows.len());
    let mut dispositions = Vec::new();
    for (machine_symbol, state_symbol) in states {
        let mut events = Vec::new();
        for (index, resource) in direct.iter().enumerate().filter(|(_, resource)| {
            resource.machine_symbol == machine_symbol && resource.state_symbol == state_symbol
        }) {
            let (activation, weakening) = exact_resource_lifecycle_handles(
                flow,
                machine_symbol,
                state_symbol,
                resource.loan,
            )?;
            events.push(LifecycleEvent {
                boundary: activation_boundary(
                    flow.borrow_lifetimes.activations.get(activation).source,
                )?,
                resource: ParentResourceIndex::Direct(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Activate { activation },
            });
            let weakening_fact = flow.borrow_lifetimes.weakenings.get(weakening);
            events.push(LifecycleEvent {
                boundary: weakening_event_boundary(weakening_fact.source, weakening_fact.reason)?,
                resource: ParentResourceIndex::Direct(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Weaken { weakening },
            });
        }
        for (index, resource) in reborrows.iter().enumerate().filter(|(_, resource)| {
            resource.machine_symbol == machine_symbol && resource.state_symbol == state_symbol
        }) {
            events.push(LifecycleEvent {
                boundary: activation_boundary(resource.activation_source)?,
                resource: ParentResourceIndex::Reborrow(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Activate {
                    activation: resource.child_activation,
                },
            });
            events.push(LifecycleEvent {
                boundary: weakening_event_boundary(
                    resource.weakening_source,
                    resource.weakening_reason,
                )?,
                resource: ParentResourceIndex::Reborrow(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Weaken {
                    weakening: resource.child_weakening,
                },
            });
        }
        events.sort_by_key(|event| (event.boundary, event.loan_order));

        let mut start = 0usize;
        while start < events.len() {
            let boundary = events[start].boundary;
            let mut end = start + 1;
            while end < events.len() && events[end].boundary == boundary {
                end += 1;
            }
            let batch = &events[start..end];
            if boundary.phase == LifecyclePhase::Activation {
                apply_activation_batch(&mut statuses, batch, installation, reborrows)?;
            } else {
                let completed = apply_weakening_batch(&mut statuses, batch)?;
                let snapshot = statuses.clone();
                let completed_resources = completed
                    .iter()
                    .map(|(index, _)| ParentResourceIndex::Reborrow(*index))
                    .collect::<Vec<_>>();
                let mut updates = Vec::new();
                for (child_index, child_weakening) in completed {
                    let (draft, update) = resolve_disposition_event(
                        flow,
                        child_index,
                        child_weakening,
                        boundary,
                        &snapshot,
                        direct,
                        reborrows,
                        installation,
                        &completed_resources,
                    )?;
                    updates.push((draft.retired_parent_path.clone(), update));
                    dispositions.push(draft);
                }
                for (retired_path, update) in updates {
                    for (resource, _) in retired_path {
                        if !statuses.set(resource, EphemeralResourceStatus::Retired) {
                            return Err(reborrow_disposition_drift());
                        }
                    }
                    match update {
                        DispositionUpdate::None => {}
                        DispositionUpdate::RestoreExclusive {
                            target,
                            expected_child,
                        } => {
                            if statuses.get(target)
                                != Some(EphemeralResourceStatus::SuspendedBy {
                                    child: expected_child,
                                })
                                || !statuses.set(target, EphemeralResourceStatus::Available)
                            {
                                return Err(reborrow_disposition_drift());
                            }
                        }
                        DispositionUpdate::UpdateSharedParent { parent, status } => {
                            if !statuses.set(parent, status) {
                                return Err(reborrow_disposition_drift());
                            }
                        }
                    }
                }
            }
            start = end;
        }
    }
    Ok(dispositions)
}

fn resolve_disposition_event(
    flow: &FlowFacts,
    child_index: usize,
    child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    boundary: LifecycleBoundaryKey,
    statuses: &EphemeralStatuses,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
    completed_resources: &[ParentResourceIndex],
) -> Result<(CheckedReborrowDispositionEventDraft, DispositionUpdate), Vec<Diagnostic>> {
    let Some(child) = reborrows.get(child_index) else {
        return Err(reborrow_disposition_drift());
    };
    if child.child_weakening != child_weakening {
        return Err(reborrow_disposition_drift());
    }
    let child_resource = ParentResourceIndex::Reborrow(child_index);
    let Some(immediate_parent) = installation.get(child_index).copied() else {
        return Err(reborrow_disposition_drift());
    };
    if child.access_effect != CheckedReborrowAccessEffect::ExclusiveSuspension {
        return resolve_shared_disposition_event(
            child_index,
            child_weakening,
            boundary,
            statuses,
            child,
            immediate_parent,
            completed_resources,
        );
    }
    let mut retired_parent_path = Vec::new();
    let (disposition, final_target, update_target) = match statuses.get(immediate_parent) {
        Some(EphemeralResourceStatus::SuspendedBy { child }) if child == child_resource => (
            CheckedReborrowResourceDisposition::Reactivate,
            DispositionTargetIndex::ParentResource(immediate_parent),
            DispositionUpdate::RestoreExclusive {
                target: immediate_parent,
                expected_child: child_resource,
            },
        ),
        Some(EphemeralResourceStatus::RetiredWhileSuspended { child, weakening })
            if child == child_resource =>
        {
            let mut retired = immediate_parent;
            let mut retired_weakening = weakening;
            loop {
                retired_parent_path.push((retired, retired_weakening));
                let retired_fact = flow.borrow_lifetimes.weakenings.get(retired_weakening);
                let retired_boundary =
                    weakening_event_boundary(retired_fact.source, retired_fact.reason)?;
                if retired_boundary > boundary {
                    return Err(reborrow_disposition_drift());
                }
                if retired_boundary == boundary {
                    let final_target = match retired {
                        ParentResourceIndex::Direct(index) => {
                            if direct.get(index).is_none() {
                                return Err(reborrow_disposition_drift());
                            }
                            DispositionTargetIndex::DirectRootLifetime(index)
                        }
                        ParentResourceIndex::Reborrow(_) => {
                            DispositionTargetIndex::ParentResource(retired)
                        }
                    };
                    let disposition =
                        closing_disposition(boundary, retired_boundary, &final_target)?;
                    break (disposition, final_target, DispositionUpdate::None);
                }
                match retired {
                    ParentResourceIndex::Direct(index) => {
                        if direct.get(index).is_none() {
                            return Err(reborrow_disposition_drift());
                        }
                        let final_target = DispositionTargetIndex::DirectRootLifetime(index);
                        let disposition = if boundary.phase == LifecyclePhase::StateExit {
                            closing_disposition(boundary, retired_boundary, &final_target)?
                        } else {
                            CheckedReborrowResourceDisposition::CascadeThroughRetiredParent
                        };
                        break (disposition, final_target, DispositionUpdate::None);
                    }
                    ParentResourceIndex::Reborrow(index) => {
                        let Some(next) = installation.get(index).copied() else {
                            return Err(reborrow_disposition_drift());
                        };
                        match statuses.get(next) {
                            Some(EphemeralResourceStatus::SuspendedBy { child })
                                if child == retired =>
                            {
                                break (
                                    CheckedReborrowResourceDisposition::CascadeThroughRetiredParent,
                                    DispositionTargetIndex::ParentResource(next),
                                    DispositionUpdate::RestoreExclusive {
                                        target: next,
                                        expected_child: retired,
                                    },
                                );
                            }
                            Some(EphemeralResourceStatus::RetiredWhileSuspended {
                                child,
                                weakening,
                            }) if child == retired => {
                                retired = next;
                                retired_weakening = weakening;
                            }
                            _ => return Err(reborrow_disposition_drift()),
                        }
                    }
                }
            }
        }
        _ => return Err(reborrow_disposition_drift()),
    };
    let boundary_source = child.weakening_source;
    let boundary_phase = boundary.phase;
    Ok((
        CheckedReborrowDispositionEventDraft {
            machine_symbol: child.machine_symbol,
            state_symbol: child.state_symbol,
            child_loan: child.loan,
            child_resource: child_index,
            child_activation: child.child_activation,
            child_weakening,
            parent_loan: child.parent_loan,
            parent_resource: immediate_parent,
            boundary_source,
            boundary_phase,
            shared_cohort: Vec::new(),
            retired_parent_path,
            final_target,
            disposition,
            restored_call: Default::default(),
            restored_borrow_call: Default::default(),
            restored_call_access: Default::default(),
        },
        update_target,
    ))
}

fn closing_disposition(
    boundary: LifecycleBoundaryKey,
    retired_boundary: LifecycleBoundaryKey,
    final_target: &DispositionTargetIndex,
) -> Result<CheckedReborrowResourceDisposition, Vec<Diagnostic>> {
    if boundary.phase == LifecyclePhase::StateExit
        && matches!(final_target, DispositionTargetIndex::DirectRootLifetime(_))
    {
        return Ok(CheckedReborrowResourceDisposition::StateExitDirectRootHandoff);
    }
    if retired_boundary == boundary {
        return Ok(CheckedReborrowResourceDisposition::SameBoundaryLineageClosure);
    }
    Err(reborrow_disposition_drift())
}

fn resolve_shared_disposition_event(
    child_index: usize,
    child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    boundary: LifecycleBoundaryKey,
    statuses: &EphemeralStatuses,
    child: &CheckedReborrowLoanResourceDraft,
    immediate_parent: ParentResourceIndex,
    completed_resources: &[ParentResourceIndex],
) -> Result<(CheckedReborrowDispositionEventDraft, DispositionUpdate), Vec<Diagnostic>> {
    let child_resource = ParentResourceIndex::Reborrow(child_index);
    let (disposition, shared_cohort, update) = match child.access_effect {
        CheckedReborrowAccessEffect::SharedRelease => {
            if !matches!(
                statuses.get(immediate_parent),
                Some(EphemeralResourceStatus::Available | EphemeralResourceStatus::Retired)
            ) {
                return Err(reborrow_disposition_drift());
            }
            (
                CheckedReborrowResourceDisposition::SharedRelease,
                vec![child_index],
                DispositionUpdate::None,
            )
        }
        CheckedReborrowAccessEffect::SharedFreeze => {
            let (cohort, parent_retired, parent_weakening) = match statuses.get(immediate_parent) {
                Some(EphemeralResourceStatus::SharedFrozenBy { children }) => {
                    (children, false, None)
                }
                Some(EphemeralResourceStatus::RetiredWhileSharedFrozen {
                    children,
                    weakening,
                }) => (children, true, Some(weakening)),
                _ => return Err(reborrow_disposition_drift()),
            };
            if !cohort.contains(&child_resource) {
                return Err(reborrow_disposition_drift());
            }
            let ending = cohort
                .iter()
                .copied()
                .filter(|member| completed_resources.contains(member))
                .collect::<Vec<_>>();
            let Some(last_ending) = ending.last().copied() else {
                return Err(reborrow_disposition_drift());
            };
            let remaining = cohort
                .iter()
                .copied()
                .filter(|member| !ending.contains(member))
                .collect::<Vec<_>>();
            let is_batch_leader = child_resource == last_ending;
            let restores_parent = is_batch_leader && remaining.is_empty() && !parent_retired;
            let disposition = if restores_parent {
                CheckedReborrowResourceDisposition::RestoreSharedCohort
            } else {
                CheckedReborrowResourceDisposition::SharedRelease
            };
            let update = if !is_batch_leader {
                DispositionUpdate::None
            } else {
                let status = if remaining.is_empty() {
                    if parent_retired {
                        EphemeralResourceStatus::Retired
                    } else {
                        EphemeralResourceStatus::Available
                    }
                } else if let Some(weakening) = parent_weakening {
                    EphemeralResourceStatus::RetiredWhileSharedFrozen {
                        children: remaining,
                        weakening,
                    }
                } else {
                    EphemeralResourceStatus::SharedFrozenBy {
                        children: remaining,
                    }
                };
                DispositionUpdate::UpdateSharedParent {
                    parent: immediate_parent,
                    status,
                }
            };
            let shared_cohort = cohort
                .iter()
                .map(|member| match member {
                    ParentResourceIndex::Reborrow(index) => Ok(*index),
                    ParentResourceIndex::Direct(_) => Err(reborrow_disposition_drift()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            (disposition, shared_cohort, update)
        }
        CheckedReborrowAccessEffect::ExclusiveSuspension => unreachable!(),
    };
    Ok((
        CheckedReborrowDispositionEventDraft {
            machine_symbol: child.machine_symbol,
            state_symbol: child.state_symbol,
            child_loan: child.loan,
            child_resource: child_index,
            child_activation: child.child_activation,
            child_weakening,
            parent_loan: child.parent_loan,
            parent_resource: immediate_parent,
            boundary_source: child.weakening_source,
            boundary_phase: boundary.phase,
            shared_cohort,
            retired_parent_path: Vec::new(),
            final_target: DispositionTargetIndex::ParentResource(immediate_parent),
            disposition,
            restored_call: Default::default(),
            restored_borrow_call: Default::default(),
            restored_call_access: Default::default(),
        },
        update,
    ))
}

fn exact_shared_cohort_observation(
    program: &dyn ::validation::CheckingInput,
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
    let Some(target_state) = program.state_by_symbol(observation.target_symbol) else {
        return false;
    };
    let Some(target_machine) = program.machines().iter().find(|machine| {
        typed_trees::state::state_by_symbol(program.machine_states(machine), target_state.symbol)
            .is_some()
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
                !matches!(
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

/// Bind the checked post-restoration mutating call onto each qualifying
/// disposition event. A Reactivate or RestoreSharedCohort event already names
/// the restored parent; this pass additionally requires the boundary's exact
/// receiver-free, single-argument mutating call, the whole-root mutable access
/// onto the parent, the exact constraint roster, and the call's one mutated
/// place. Unsupported shapes stay unbound rather than receiving inferred
/// authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_restored_call_uses(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    flow: &checked_trees::FlowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    dispositions: &mut [CheckedReborrowDispositionEventDraft],
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    for event in dispositions.iter_mut() {
        // The event's disposition selects the exact shape: Reactivate
        // requires an exclusive suspension, RestoreSharedCohort a bounded
        // shared-freeze cohort carrying every child of the parent.
        let restored = match event.disposition {
            CheckedReborrowResourceDisposition::Reactivate => true,
            CheckedReborrowResourceDisposition::RestoreSharedCohort => false,
            _ => continue,
        };
        let ParentResourceIndex::Direct(parent_index) = event.parent_resource else {
            continue;
        };
        let Some(parent) = direct.get(parent_index) else {
            return Err(reborrow_restored_call_use_drift());
        };
        let Some(child) = reborrows.get(event.child_resource) else {
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
        if restored != exclusive_reactivation || !restored && !bounded_shared_freeze {
            continue;
        }
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
            || event.boundary_phase != CheckedBorrowResourceLifecyclePhase::LastUseExpired
            || event.boundary_source != child.weakening_source
            || event.parent_loan != parent.loan
            || !event.retired_parent_path.is_empty()
            || (restored && !event.shared_cohort.is_empty())
            || (!restored
                && event.shared_cohort
                    != shared_cohort
                        .iter()
                        .map(|(index, _)| *index)
                        .collect::<Vec<_>>())
            || event.final_target != DispositionTargetIndex::ParentResource(event.parent_resource)
        {
            continue;
        }
        let FlowInvalidationSource::Statement { statement_index } = child.weakening_source else {
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
        let Some(target_state) = program.state_by_symbol(call.target_symbol) else {
            continue;
        };
        let Some(target_machine) = program.machines().iter().find(|machine| {
            typed_trees::state::state_by_symbol(
                program.machine_states(machine),
                target_state.symbol,
            )
            .is_some()
        }) else {
            continue;
        };
        // A nominal type qualifier is source lookup, not a runtime receiver.
        // Keep true instance-receiver calls outside this binding.
        if call.has_receiver && target_machine.attached_data_symbol != call.receiver_symbol {
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
            .collect::<Vec<_>>();
        let [parameter] = parameters.as_slice() else {
            continue;
        };
        if !parameter.is_mutable {
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
            || access.kind != checked_trees::BorrowAccessKind::Mutable
            || access.end_before_access.is_some()
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
            .filter(|constraint| {
                constraint.kind
                    == checked_trees::FlowConstraintKind::BorrowLoan { loan: parent.loan }
            })
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
        let [_parent_entry_constraint] = parent_constraints.as_slice() else {
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
            None,
        );
        let [mutated_place] = mutated_places.as_slice() else {
            continue;
        };
        if mutated_place.root != facts::PlaceRoot::Symbol(parent.owner_symbol)
            || !mutated_place.segments.is_empty()
        {
            continue;
        }

        event.restored_call = *call_handle;
        event.restored_borrow_call = *borrow_call_handle;
        event.restored_call_access = access_handle;
    }
    Ok(())
}
