//! Resource lifecycle phases, boundaries, events and the ephemeral statuses
//! the activation and weakening batches update.

use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowLoanResourceDraft, ParentResourceIndex,
};
use crate::checks::borrows::resources::resource_reconstruction::span_handle;
use crate::checks::borrows::resources::retained_validation::reborrow_disposition_drift;
use checked_trees::{
    BorrowLoanFact, CheckedBorrowResourceLifecyclePhase, CheckedReborrowAccessEffect, FlowFacts,
    FlowInvalidationSource,
};
use diagnostics::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LifecyclePhase {
    LastUseExpired,
    LocalReassigned,
    Activation,
    StateExit,
}

impl LifecyclePhase {
    pub(crate) fn retained(self) -> CheckedBorrowResourceLifecyclePhase {
        match self {
            Self::LastUseExpired => CheckedBorrowResourceLifecyclePhase::LastUseExpired,
            Self::LocalReassigned => CheckedBorrowResourceLifecyclePhase::LocalReassigned,
            Self::Activation => CheckedBorrowResourceLifecyclePhase::Activation,
            Self::StateExit => CheckedBorrowResourceLifecyclePhase::StateExit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LifecycleBoundaryKey {
    statement_index: usize,
    pub(crate) phase: LifecyclePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleEventKind {
    Activate {
        activation: arena::Handle<checked_trees::FlowBorrowActivationFact>,
    },
    Weaken {
        weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LifecycleEvent {
    pub(crate) boundary: LifecycleBoundaryKey,
    pub(crate) resource: ParentResourceIndex,
    pub(crate) loan_order: u32,
    pub(crate) kind: LifecycleEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EphemeralResourceStatus {
    Available,
    SharedFrozenBy {
        children: Vec<ParentResourceIndex>,
    },
    SuspendedBy {
        child: ParentResourceIndex,
    },
    RetiredWhileSharedFrozen {
        children: Vec<ParentResourceIndex>,
        weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    },
    RetiredWhileSuspended {
        child: ParentResourceIndex,
        weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    },
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DispositionUpdate {
    None,
    RestoreExclusive {
        target: ParentResourceIndex,
        expected_child: ParentResourceIndex,
    },
    UpdateSharedParent {
        parent: ParentResourceIndex,
        status: EphemeralResourceStatus,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct EphemeralStatuses {
    direct: Vec<Option<EphemeralResourceStatus>>,
    reborrows: Vec<Option<EphemeralResourceStatus>>,
}

impl EphemeralStatuses {
    pub(crate) fn new(direct: usize, reborrows: usize) -> Self {
        Self {
            direct: vec![None; direct],
            reborrows: vec![None; reborrows],
        }
    }

    pub(crate) fn get(&self, resource: ParentResourceIndex) -> Option<EphemeralResourceStatus> {
        match resource {
            ParentResourceIndex::Direct(index) => self.direct.get(index).cloned().flatten(),
            ParentResourceIndex::Reborrow(index) => self.reborrows.get(index).cloned().flatten(),
        }
    }

    pub(crate) fn set(
        &mut self,
        resource: ParentResourceIndex,
        status: EphemeralResourceStatus,
    ) -> bool {
        let slot = match resource {
            ParentResourceIndex::Direct(index) => self.direct.get_mut(index),
            ParentResourceIndex::Reborrow(index) => self.reborrows.get_mut(index),
        };
        let Some(slot) = slot else {
            return false;
        };
        *slot = Some(status);
        true
    }
}

pub(crate) fn exact_resource_lifecycle_handles(
    flow: &FlowFacts,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    loan: arena::Handle<BorrowLoanFact>,
) -> Result<
    (
        arena::Handle<checked_trees::FlowBorrowActivationFact>,
        arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    ),
    Vec<Diagnostic>,
> {
    let Some(state) = flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    }) else {
        return Err(reborrow_disposition_drift());
    };
    let activations = flow
        .borrow_lifetimes
        .activations
        .span_or_empty(state.borrow_activations)
        .iter()
        .enumerate()
        .filter(|(_, event)| event.loan == loan)
        .filter_map(|(offset, _)| span_handle(state.borrow_activations, offset))
        .collect::<Vec<_>>();
    let weakenings = flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(state.borrow_weakenings)
        .iter()
        .enumerate()
        .filter(|(_, event)| event.loan == loan)
        .filter_map(|(offset, _)| span_handle(state.borrow_weakenings, offset))
        .collect::<Vec<_>>();
    let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice()) else {
        return Err(reborrow_disposition_drift());
    };
    Ok((*activation, *weakening))
}

pub(crate) fn activation_boundary(
    source: FlowInvalidationSource,
) -> Result<LifecycleBoundaryKey, Vec<Diagnostic>> {
    let FlowInvalidationSource::Statement { statement_index } = source else {
        return Err(reborrow_disposition_drift());
    };
    Ok(LifecycleBoundaryKey {
        statement_index,
        phase: LifecyclePhase::Activation,
    })
}

pub(crate) fn weakening_event_boundary(
    source: FlowInvalidationSource,
    reason: checked_trees::FlowBorrowWeakeningReason,
) -> Result<LifecycleBoundaryKey, Vec<Diagnostic>> {
    let FlowInvalidationSource::Statement { statement_index } = source else {
        return Err(reborrow_disposition_drift());
    };
    let phase = match reason {
        checked_trees::FlowBorrowWeakeningReason::LastUseExpired => LifecyclePhase::LastUseExpired,
        checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
            LifecyclePhase::LocalReassigned
        }
        checked_trees::FlowBorrowWeakeningReason::StateExit => LifecyclePhase::StateExit,
    };
    Ok(LifecycleBoundaryKey {
        statement_index,
        phase,
    })
}

pub(crate) fn apply_activation_batch(
    statuses: &mut EphemeralStatuses,
    batch: &[LifecycleEvent],
    installation: &[ParentResourceIndex],
    reborrows: &[CheckedReborrowLoanResourceDraft],
) -> Result<(), Vec<Diagnostic>> {
    for event in batch {
        let LifecycleEventKind::Activate { activation } = event.kind else {
            return Err(reborrow_disposition_drift());
        };
        if !activation.is_valid() || statuses.get(event.resource).is_some() {
            return Err(reborrow_disposition_drift());
        }
        if let ParentResourceIndex::Reborrow(child_index) = event.resource {
            let Some(parent) = installation.get(child_index).copied() else {
                return Err(reborrow_disposition_drift());
            };
            let Some(child) = reborrows.get(child_index) else {
                return Err(reborrow_disposition_drift());
            };
            let parent_status = statuses.get(parent);
            let next_parent = match child.access_effect {
                CheckedReborrowAccessEffect::SharedRelease => {
                    if parent_status != Some(EphemeralResourceStatus::Available) {
                        return Err(reborrow_disposition_drift());
                    }
                    None
                }
                CheckedReborrowAccessEffect::SharedFreeze => match parent_status {
                    Some(EphemeralResourceStatus::Available) => {
                        Some(EphemeralResourceStatus::SharedFrozenBy {
                            children: vec![event.resource],
                        })
                    }
                    Some(EphemeralResourceStatus::SharedFrozenBy { mut children }) => {
                        if children.contains(&event.resource) {
                            return Err(reborrow_disposition_drift());
                        }
                        children.push(event.resource);
                        Some(EphemeralResourceStatus::SharedFrozenBy { children })
                    }
                    _ => return Err(reborrow_disposition_drift()),
                },
                CheckedReborrowAccessEffect::ExclusiveSuspension => {
                    if parent_status != Some(EphemeralResourceStatus::Available) {
                        return Err(reborrow_disposition_drift());
                    }
                    Some(EphemeralResourceStatus::SuspendedBy {
                        child: event.resource,
                    })
                }
            };
            if let Some(next_parent) = next_parent
                && !statuses.set(parent, next_parent)
            {
                return Err(reborrow_disposition_drift());
            }
        }
        if !statuses.set(event.resource, EphemeralResourceStatus::Available) {
            return Err(reborrow_disposition_drift());
        }
    }
    Ok(())
}

pub(crate) fn apply_weakening_batch(
    statuses: &mut EphemeralStatuses,
    batch: &[LifecycleEvent],
) -> Result<Vec<(usize, arena::Handle<checked_trees::FlowBorrowWeakeningFact>)>, Vec<Diagnostic>> {
    let mut completed = Vec::new();
    for event in batch {
        let LifecycleEventKind::Weaken { weakening } = event.kind else {
            return Err(reborrow_disposition_drift());
        };
        let next = match statuses.get(event.resource) {
            Some(EphemeralResourceStatus::Available) => {
                if let ParentResourceIndex::Reborrow(index) = event.resource {
                    completed.push((index, weakening));
                }
                EphemeralResourceStatus::Retired
            }
            Some(EphemeralResourceStatus::SuspendedBy { child }) => {
                EphemeralResourceStatus::RetiredWhileSuspended { child, weakening }
            }
            Some(EphemeralResourceStatus::SharedFrozenBy { children }) => {
                EphemeralResourceStatus::RetiredWhileSharedFrozen {
                    children,
                    weakening,
                }
            }
            None
            | Some(EphemeralResourceStatus::RetiredWhileSharedFrozen { .. })
            | Some(EphemeralResourceStatus::RetiredWhileSuspended { .. })
            | Some(EphemeralResourceStatus::Retired) => {
                return Err(reborrow_disposition_drift());
            }
        };
        if !statuses.set(event.resource, next) {
            return Err(reborrow_disposition_drift());
        }
    }
    Ok(completed)
}

pub(crate) fn weakening_boundary_key(
    source: FlowInvalidationSource,
    reason: checked_trees::FlowBorrowWeakeningReason,
) -> Option<(usize, u8)> {
    let FlowInvalidationSource::Statement { statement_index } = source else {
        return None;
    };
    let phase = match reason {
        checked_trees::FlowBorrowWeakeningReason::LastUseExpired => 0,
        checked_trees::FlowBorrowWeakeningReason::LocalReassigned => 1,
        checked_trees::FlowBorrowWeakeningReason::StateExit => 2,
    };
    Some((statement_index, phase))
}
