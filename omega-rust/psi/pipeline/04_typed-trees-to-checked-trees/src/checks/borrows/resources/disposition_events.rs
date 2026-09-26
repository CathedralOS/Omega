//! Planning and resolving reborrow disposition events.

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
use crate::checks::borrows::resources::retained_validation::reborrow_disposition_drift;
use checked_trees::{
    CheckedDirectBorrowLoanResource, CheckedReborrowAccessEffect,
    CheckedReborrowResourceDisposition, FlowFacts,
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
    let boundary_phase = boundary.phase.retained();
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
            boundary_phase: boundary.phase.retained(),
            shared_cohort,
            retired_parent_path: Vec::new(),
            final_target: DispositionTargetIndex::ParentResource(immediate_parent),
            disposition,
        },
        update,
    ))
}
