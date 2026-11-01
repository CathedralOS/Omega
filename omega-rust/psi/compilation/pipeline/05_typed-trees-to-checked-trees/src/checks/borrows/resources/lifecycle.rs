//! Resource lifecycle phases, boundaries, events and the ephemeral statuses
//! the activation and weakening batches update.

use crate::checks::borrows::resources::reborrow_disposition_drift;
use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowLoanResourceDraft, ParentResourceIndex,
};
use checked_trees::{
    BorrowLoanFact, CheckedBorrowResourceLifecyclePhase, FlowFacts, FlowInvalidationSource,
};
use diagnostics::Diagnostic;

pub(crate) type LifecyclePhase = CheckedBorrowResourceLifecyclePhase;

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
    BorrowedBy {
        children: Vec<ParentResourceIndex>,
    },
    RetiredWhileBorrowed {
        children: Vec<ParentResourceIndex>,
        weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    },
    Retired,
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

/// The durable lifecycle rows of one loan: its activation and weakening.
pub(crate) type LoanLifecycleRows = (
    arena::Handle<checked_trees::FlowBorrowActivationFact>,
    arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
);

/// The one activation and one weakening row of `loan` at `flow_state`. Every
/// checked borrow-resource reconstruction joins the lifecycle ledger through
/// this reader, so a ledger that holds no single pair — or an activation that
/// does not start at the loan's own formation statement — is refused here,
/// never reconstructed differently per consumer.
pub(crate) fn exact_loan_lifecycle_rows(
    flow: &FlowFacts,
    flow_state: &checked_trees::FlowStateFact,
    loan: arena::Handle<BorrowLoanFact>,
    formation_statement: usize,
) -> Result<LoanLifecycleRows, Diagnostic> {
    let activations = flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow_state.borrow_activations)
        .iter()
        .enumerate()
        .filter(|(_, event)| event.loan == loan)
        .filter_map(|(offset, _)| span_handle(flow_state.borrow_activations, offset))
        .collect::<Vec<_>>();
    let weakenings = flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow_state.borrow_weakenings)
        .iter()
        .enumerate()
        .filter(|(_, event)| event.loan == loan)
        .filter_map(|(offset, _)| span_handle(flow_state.borrow_weakenings, offset))
        .collect::<Vec<_>>();
    let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice()) else {
        return Err(Diagnostic::error(
            "checked borrow resource requires exactly one activation and one weakening",
        ));
    };
    if flow.borrow_lifetimes.activations.get(*activation).source
        != (FlowInvalidationSource::Statement {
            statement_index: formation_statement,
        })
    {
        return Err(Diagnostic::error(
            "checked borrow resource activation drifted from loan formation",
        ));
    }
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
            // The ledger retains descent and closure, not relational admission.
            // Siblings may suspend disjoint projections of the same parent;
            // compatibility judges their captured places separately. Recording
            // a second child must neither restore nor duplicate whole-parent
            // authority. Shared children also remain in the closure graph.
            if child.parent_access.direct_reborrow_effect(&child.access)
                != Some(child.access_effect)
            {
                return Err(reborrow_disposition_drift());
            }
            let mut children = match statuses.get(parent) {
                Some(EphemeralResourceStatus::Available) => Vec::new(),
                Some(EphemeralResourceStatus::BorrowedBy { children }) => children,
                _ => return Err(reborrow_disposition_drift()),
            };
            if children.contains(&event.resource) {
                return Err(reborrow_disposition_drift());
            }
            children.push(event.resource);
            if !statuses.set(parent, EphemeralResourceStatus::BorrowedBy { children }) {
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
            Some(EphemeralResourceStatus::BorrowedBy { children }) => {
                EphemeralResourceStatus::RetiredWhileBorrowed {
                    children,
                    weakening,
                }
            }
            None
            | Some(EphemeralResourceStatus::RetiredWhileBorrowed { .. })
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

/// Translate a span offset into the durable handle the row carries.
pub(crate) fn span_handle<T>(
    span: arena::HandleSpan<T>,
    offset: usize,
) -> Option<arena::Handle<T>> {
    u32::try_from(offset)
        .ok()
        .map(|offset| span.handle_at(offset))
}
