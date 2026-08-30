use psi_checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, CheckFacts,
    CheckedBorrowResourceDispositionTarget, CheckedBorrowResourceLifecyclePhase,
    CheckedDirectBorrowLoanResource, CheckedDirectBorrowParentLifetime,
    CheckedDirectBorrowRestorationObligation, CheckedParentBorrowResource,
    CheckedReborrowAccessEffect, CheckedReborrowLoanResource, CheckedReborrowParentEndStatus,
    CheckedReborrowParentSuspensionBoundary, CheckedReborrowResourceDisposition,
    CheckedReborrowResourceDispositionEvent, CheckedReborrowRestorationObligation,
    CheckedRetiredParentResourceDispositionStep, FlowFacts, FlowInvalidationSource,
    ParentLexicalStatusAtChildEnd,
};
use psi_diagnostics::Diagnostic;

/// Populate the checked-only direct-root and direct-reborrow resource closures
/// before ordinary checked-fact replay.
pub(super) fn initialize_checked_direct_borrow_resources(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let direct = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    let reborrows = reconstruct_reborrow_resource_drafts(&facts.borrow, &facts.flow)?;
    let installation = plan_resource_installation(&direct, &reborrows)?;
    let dispositions =
        plan_reborrow_disposition_events(&facts.flow, &direct, &reborrows, &installation)?;
    install_borrow_resources(
        &mut facts.borrow,
        direct,
        &reborrows,
        &installation,
        &dispositions,
    );
    Ok(())
}

/// Independently replay every retained resource from the authoritative loan
/// and flow-lifetime ledgers, then transactionally rebuild both arenas with
/// remapped typed parent handles. The rows never participate in admission.
pub(super) fn replay_checked_direct_borrow_resources(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let expected_direct = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    let expected_reborrows = reconstruct_reborrow_resource_drafts(&facts.borrow, &facts.flow)?;
    let retained = facts
        .borrow
        .direct_loan_resources
        .iter()
        .map(|(_, resource)| resource.clone())
        .collect::<Vec<_>>();
    if retained != expected_direct {
        return Err(vec![Diagnostic::error(
            "checked direct-root borrow resource closure drifted from independent replay",
        )]);
    }
    validate_retained_reborrow_resources(&facts.borrow, &expected_reborrows)?;
    let installation = plan_resource_installation(&expected_direct, &expected_reborrows)?;
    let dispositions = plan_reborrow_disposition_events(
        &facts.flow,
        &expected_direct,
        &expected_reborrows,
        &installation,
    )?;
    validate_retained_disposition_events(
        &facts.borrow,
        &expected_direct,
        &expected_reborrows,
        &dispositions,
    )?;

    install_borrow_resources(
        &mut facts.borrow,
        expected_direct,
        &expected_reborrows,
        &installation,
        &dispositions,
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedReborrowLoanResourceDraft {
    loan: psi_arena::Handle<BorrowLoanFact>,
    machine_symbol: psi_symbols::SymbolHandle,
    state_symbol: psi_symbols::SymbolHandle,
    owner_symbol: psi_symbols::SymbolHandle,
    owner_path: Vec<psi_checked_trees::BorrowLoanOwnerSegment>,
    captured_place: psi_checked_trees::CapturedPlace,
    access: psi_checked_trees::BorrowAccessKind,
    parent_access: psi_checked_trees::BorrowAccessKind,
    access_effect: CheckedReborrowAccessEffect,
    activation_source: psi_checked_trees::FlowInvalidationSource,
    weakening_source: psi_checked_trees::FlowInvalidationSource,
    weakening_reason: psi_checked_trees::FlowBorrowWeakeningReason,
    parent_loan: psi_arena::Handle<BorrowLoanFact>,
    child_activation: psi_arena::Handle<psi_checked_trees::FlowBorrowActivationFact>,
    parent_entry_constraint: psi_arena::Handle<psi_checked_trees::FlowConstraintRef>,
    child_weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    parent_weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    parent_lexical_status: ParentLexicalStatusAtChildEnd,
}

impl CheckedReborrowLoanResourceDraft {
    fn close(&self, parent_resource: CheckedParentBorrowResource) -> CheckedReborrowLoanResource {
        CheckedReborrowLoanResource {
            loan: self.loan,
            machine_symbol: self.machine_symbol,
            state_symbol: self.state_symbol,
            owner_symbol: self.owner_symbol,
            owner_path: self.owner_path.clone(),
            captured_place: self.captured_place.clone(),
            access: self.access.clone(),
            parent_access: self.parent_access.clone(),
            access_effect: self.access_effect,
            activation_source: self.activation_source,
            weakening_source: self.weakening_source,
            weakening_reason: self.weakening_reason,
            parent_loan: self.parent_loan,
            parent_resource: parent_resource.clone(),
            parent_suspension: CheckedReborrowParentSuspensionBoundary {
                child_loan: self.loan,
                parent_loan: self.parent_loan,
                parent_resource: parent_resource.clone(),
                child_activation: self.child_activation,
                parent_entry_constraint: self.parent_entry_constraint,
                source: self.activation_source,
            },
            parent_end_status: CheckedReborrowParentEndStatus {
                child_loan: self.loan,
                parent_loan: self.parent_loan,
                parent_resource: parent_resource.clone(),
                child_weakening: self.child_weakening,
                parent_weakening: self.parent_weakening,
                status: self.parent_lexical_status,
            },
            restoration: CheckedReborrowRestorationObligation {
                child_loan: self.loan,
                parent_loan: self.parent_loan,
                parent_resource,
                child_weakening_source: self.weakening_source,
                child_weakening_reason: self.weakening_reason,
            },
        }
    }
}

fn validate_retained_reborrow_resources(
    borrow: &BorrowFacts,
    expected: &[CheckedReborrowLoanResourceDraft],
) -> Result<(), Vec<Diagnostic>> {
    let retained = borrow.reborrow_loan_resources.iter().collect::<Vec<_>>();
    if retained.len() != expected.len() {
        return Err(reborrow_resource_drift());
    }

    let mut prior_reborrows = Vec::new();
    for ((resource_handle, retained), draft) in retained.into_iter().zip(expected) {
        let parent_resource = retained_parent_resource(borrow, draft.parent_loan, &prior_reborrows)
            .ok_or_else(reborrow_resource_drift)?;
        if retained != &draft.close(parent_resource) {
            return Err(reborrow_resource_drift());
        }
        prior_reborrows.push((draft.loan, resource_handle));
    }
    Ok(())
}

fn retained_parent_resource(
    borrow: &BorrowFacts,
    parent_loan: psi_arena::Handle<BorrowLoanFact>,
    prior_reborrows: &[(
        psi_arena::Handle<BorrowLoanFact>,
        psi_arena::Handle<CheckedReborrowLoanResource>,
    )],
) -> Option<CheckedParentBorrowResource> {
    match &borrow.loans.get(parent_loan).lineage {
        BorrowLoanLineage::DirectRoot => {
            let mut matches = borrow
                .direct_loan_resources
                .iter()
                .filter(|(_, resource)| resource.loan == parent_loan);
            let handle = matches.next()?.0;
            matches
                .next()
                .is_none()
                .then_some(CheckedParentBorrowResource::DirectRoot { resource: handle })
        }
        BorrowLoanLineage::Reborrow { .. } => {
            prior_reborrows.iter().find_map(|(loan, resource)| {
                (*loan == parent_loan).then_some(CheckedParentBorrowResource::Reborrow {
                    resource: *resource,
                })
            })
        }
        BorrowLoanLineage::UnretainedDerived => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentResourceIndex {
    Direct(usize),
    Reborrow(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DispositionTargetIndex {
    ParentResource(ParentResourceIndex),
    DirectRootLifetime(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedReborrowDispositionEventDraft {
    machine_symbol: psi_symbols::SymbolHandle,
    state_symbol: psi_symbols::SymbolHandle,
    child_loan: psi_arena::Handle<BorrowLoanFact>,
    child_resource: usize,
    child_activation: psi_arena::Handle<psi_checked_trees::FlowBorrowActivationFact>,
    child_weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    parent_loan: psi_arena::Handle<BorrowLoanFact>,
    parent_resource: ParentResourceIndex,
    boundary_source: FlowInvalidationSource,
    boundary_phase: CheckedBorrowResourceLifecyclePhase,
    shared_cohort: Vec<usize>,
    retired_parent_path: Vec<(
        ParentResourceIndex,
        psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    )>,
    final_target: DispositionTargetIndex,
    disposition: CheckedReborrowResourceDisposition,
}

impl CheckedReborrowDispositionEventDraft {
    fn close(
        &self,
        borrow: &BorrowFacts,
        handles: &ResourceHandles,
    ) -> CheckedReborrowResourceDispositionEvent {
        CheckedReborrowResourceDispositionEvent {
            machine_symbol: self.machine_symbol,
            state_symbol: self.state_symbol,
            child_loan: self.child_loan,
            child_resource: handles.reborrows[self.child_resource],
            child_activation: self.child_activation,
            child_weakening: self.child_weakening,
            parent_loan: self.parent_loan,
            parent_resource: handles.parent(self.parent_resource),
            boundary_source: self.boundary_source,
            boundary_phase: self.boundary_phase,
            shared_cohort: self
                .shared_cohort
                .iter()
                .map(|index| handles.reborrows[*index])
                .collect(),
            retired_parent_path: self
                .retired_parent_path
                .iter()
                .map(
                    |(resource, weakening)| CheckedRetiredParentResourceDispositionStep {
                        resource: handles.parent(*resource),
                        weakening: *weakening,
                    },
                )
                .collect(),
            final_target: match self.final_target {
                DispositionTargetIndex::ParentResource(resource) => {
                    CheckedBorrowResourceDispositionTarget::ParentResource(handles.parent(resource))
                }
                DispositionTargetIndex::DirectRootLifetime(index) => {
                    CheckedBorrowResourceDispositionTarget::DirectRootLifetime(
                        borrow
                            .direct_loan_resources
                            .get(handles.direct[index])
                            .parent_lifetime
                            .clone(),
                    )
                }
            },
            disposition: self.disposition,
        }
    }
}

#[derive(Debug)]
struct ResourceHandles {
    direct: Vec<psi_arena::Handle<CheckedDirectBorrowLoanResource>>,
    reborrows: Vec<psi_arena::Handle<CheckedReborrowLoanResource>>,
}

impl ResourceHandles {
    fn parent(&self, index: ParentResourceIndex) -> CheckedParentBorrowResource {
        match index {
            ParentResourceIndex::Direct(index) => CheckedParentBorrowResource::DirectRoot {
                resource: self.direct[index],
            },
            ParentResourceIndex::Reborrow(index) => CheckedParentBorrowResource::Reborrow {
                resource: self.reborrows[index],
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LifecyclePhase {
    LastUseExpired,
    LocalReassigned,
    Activation,
    StateExit,
}

impl LifecyclePhase {
    fn retained(self) -> CheckedBorrowResourceLifecyclePhase {
        match self {
            Self::LastUseExpired => CheckedBorrowResourceLifecyclePhase::LastUseExpired,
            Self::LocalReassigned => CheckedBorrowResourceLifecyclePhase::LocalReassigned,
            Self::Activation => CheckedBorrowResourceLifecyclePhase::Activation,
            Self::StateExit => CheckedBorrowResourceLifecyclePhase::StateExit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LifecycleBoundaryKey {
    statement_index: usize,
    phase: LifecyclePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleEventKind {
    Activate {
        activation: psi_arena::Handle<psi_checked_trees::FlowBorrowActivationFact>,
    },
    Weaken {
        weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecycleEvent {
    boundary: LifecycleBoundaryKey,
    resource: ParentResourceIndex,
    loan_order: u32,
    kind: LifecycleEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EphemeralResourceStatus {
    Available,
    SharedFrozenBy {
        children: Vec<ParentResourceIndex>,
    },
    SuspendedBy {
        child: ParentResourceIndex,
    },
    RetiredWhileSharedFrozen {
        children: Vec<ParentResourceIndex>,
        weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    },
    RetiredWhileSuspended {
        child: ParentResourceIndex,
        weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    },
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DispositionUpdate {
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
struct EphemeralStatuses {
    direct: Vec<Option<EphemeralResourceStatus>>,
    reborrows: Vec<Option<EphemeralResourceStatus>>,
}

impl EphemeralStatuses {
    fn new(direct: usize, reborrows: usize) -> Self {
        Self {
            direct: vec![None; direct],
            reborrows: vec![None; reborrows],
        }
    }

    fn get(&self, resource: ParentResourceIndex) -> Option<EphemeralResourceStatus> {
        match resource {
            ParentResourceIndex::Direct(index) => self.direct.get(index).cloned().flatten(),
            ParentResourceIndex::Reborrow(index) => self.reborrows.get(index).cloned().flatten(),
        }
    }

    fn set(&mut self, resource: ParentResourceIndex, status: EphemeralResourceStatus) -> bool {
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

/// Resolve the entire parent graph before either retained arena is reset.
/// Installation is therefore a purely indexed, infallible rewrite.
fn plan_resource_installation(
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

fn plan_reborrow_disposition_events(
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

fn exact_resource_lifecycle_handles(
    flow: &FlowFacts,
    machine_symbol: psi_symbols::SymbolHandle,
    state_symbol: psi_symbols::SymbolHandle,
    loan: psi_arena::Handle<BorrowLoanFact>,
) -> Result<
    (
        psi_arena::Handle<psi_checked_trees::FlowBorrowActivationFact>,
        psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
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

fn activation_boundary(
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

fn weakening_event_boundary(
    source: FlowInvalidationSource,
    reason: psi_checked_trees::FlowBorrowWeakeningReason,
) -> Result<LifecycleBoundaryKey, Vec<Diagnostic>> {
    let FlowInvalidationSource::Statement { statement_index } = source else {
        return Err(reborrow_disposition_drift());
    };
    let phase = match reason {
        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired => {
            LifecyclePhase::LastUseExpired
        }
        psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
            LifecyclePhase::LocalReassigned
        }
        psi_checked_trees::FlowBorrowWeakeningReason::StateExit => LifecyclePhase::StateExit,
    };
    Ok(LifecycleBoundaryKey {
        statement_index,
        phase,
    })
}

fn apply_activation_batch(
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

fn apply_weakening_batch(
    statuses: &mut EphemeralStatuses,
    batch: &[LifecycleEvent],
) -> Result<
    Vec<(
        usize,
        psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    )>,
    Vec<Diagnostic>,
> {
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

fn resolve_disposition_event(
    flow: &FlowFacts,
    child_index: usize,
    child_weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
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
    child_weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
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

fn validate_retained_disposition_events(
    borrow: &BorrowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    expected: &[CheckedReborrowDispositionEventDraft],
) -> Result<(), Vec<Diagnostic>> {
    let handles = ResourceHandles {
        direct: borrow
            .direct_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
        reborrows: borrow
            .reborrow_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
    };
    if handles.direct.len() != direct.len() || handles.reborrows.len() != reborrows.len() {
        return Err(reborrow_disposition_drift());
    }
    let retained = borrow
        .reborrow_disposition_events
        .iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    if retained.len() != expected.len()
        || retained
            .into_iter()
            .zip(expected)
            .any(|(retained, expected)| retained != &expected.close(borrow, &handles))
    {
        return Err(reborrow_disposition_drift());
    }
    Ok(())
}

fn reborrow_disposition_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow resource-lifecycle disposition drifted from semantic-phase replay",
    )]
}

fn install_borrow_resources(
    borrow: &mut BorrowFacts,
    direct: Vec<CheckedDirectBorrowLoanResource>,
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
    dispositions: &[CheckedReborrowDispositionEventDraft],
) {
    borrow.direct_loan_resources.reset_retain_capacity();
    borrow.reborrow_loan_resources.reset_retain_capacity();
    borrow.reborrow_disposition_events.reset_retain_capacity();

    let mut direct_handles = Vec::with_capacity(direct.len());
    for resource in direct {
        let handle = borrow.direct_loan_resources.insert(resource);
        direct_handles.push(handle);
    }

    let mut reborrow_handles: Vec<psi_arena::Handle<CheckedReborrowLoanResource>> =
        Vec::with_capacity(reborrows.len());
    for (draft, parent) in reborrows.iter().zip(installation) {
        let parent_resource = match *parent {
            ParentResourceIndex::Direct(index) => CheckedParentBorrowResource::DirectRoot {
                resource: direct_handles[index],
            },
            ParentResourceIndex::Reborrow(index) => CheckedParentBorrowResource::Reborrow {
                resource: reborrow_handles[index],
            },
        };
        let handle = borrow
            .reborrow_loan_resources
            .insert(draft.close(parent_resource));
        reborrow_handles.push(handle);
    }

    let handles = ResourceHandles {
        direct: direct_handles,
        reborrows: reborrow_handles,
    };
    for disposition in dispositions {
        borrow
            .reborrow_disposition_events
            .insert(disposition.close(borrow, &handles));
    }
}

fn reborrow_resource_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked direct-reborrow resource closure drifted from independent topological replay",
    )]
}

fn reconstruct_direct_borrow_resources(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedDirectBorrowLoanResource>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-root borrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            // Direct reborrows close in their own typed parent-resource arena;
            // every derived occurrence remains outside this root-only arena.
            if loan.lineage != BorrowLoanLineage::DirectRoot {
                continue;
            }

            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .filter(|activation| activation.loan == loan_handle)
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .filter(|weakening| weakening.loan == loan_handle)
                .collect::<Vec<_>>();
            let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice())
            else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow resource requires exactly one activation and one weakening",
                ));
                continue;
            };
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow activation drifted from loan formation",
                ));
                continue;
            }

            let parent_lifetime = CheckedDirectBorrowParentLifetime {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                root_symbol: loan.root_symbol,
            };
            let restoration = CheckedDirectBorrowRestorationObligation {
                parent: parent_lifetime.clone(),
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
            };
            resources.push(CheckedDirectBorrowLoanResource {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: psi_checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
                    segments: borrow.loan_segments(loan).to_vec(),
                },
                access: loan.kind.clone(),
                activation_source: activation.source,
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
                parent_lifetime,
                restoration,
            });
        }
    }

    if diagnostics.is_empty() {
        Ok(resources)
    } else {
        Err(diagnostics)
    }
}

fn reconstruct_reborrow_resource_drafts(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedReborrowLoanResourceDraft>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-reborrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            let BorrowLoanLineage::Reborrow { parent_loan } = &loan.lineage else {
                continue;
            };
            let parent = borrow.loans.get(*parent_loan);
            let Some(access_effect) = parent.kind.direct_reborrow_effect(&loan.kind) else {
                diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                    &parent.kind,
                    &loan.kind,
                ));
                continue;
            };
            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .enumerate()
                .filter(|(_, activation)| activation.loan == loan_handle)
                .filter_map(|(offset, activation)| {
                    span_handle(flow_state.borrow_activations, offset)
                        .map(|handle| (handle, activation))
                })
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .enumerate()
                .filter(|(_, weakening)| weakening.loan == loan_handle)
                .filter_map(|(offset, weakening)| {
                    span_handle(flow_state.borrow_weakenings, offset)
                        .map(|handle| (handle, weakening))
                })
                .collect::<Vec<_>>();
            if activations.len() != 1 || weakenings.len() != 1 {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow resource requires exactly one activation and one weakening",
                ));
                continue;
            }
            let (child_activation, activation) = activations[0];
            let (child_weakening, weakening) = weakenings[0];
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow activation drifted from loan formation",
                ));
                continue;
            }
            let parent_weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .enumerate()
                .filter(|(_, weakening)| weakening.loan == *parent_loan)
                .filter_map(|(offset, weakening)| {
                    span_handle(flow_state.borrow_weakenings, offset)
                        .map(|handle| (handle, weakening))
                })
                .collect::<Vec<_>>();
            let [(parent_weakening, parent_weakening_fact)] = parent_weakenings.as_slice() else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow parent status requires exactly one parent weakening",
                ));
                continue;
            };
            let Some(parent_lexical_status) = parent_lexical_status_at_child_end(
                parent_weakening_fact.source,
                parent_weakening_fact.reason,
                weakening.source,
                weakening.reason,
            ) else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow parent status has an unsupported weakening boundary",
                ));
                continue;
            };

            let Some(statement) = flow
                .control
                .statements
                .span_or_empty(flow_state.statements)
                .iter()
                .find(|statement| statement.statement_index == loan.statement_index)
            else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow suspension has no exact formation statement",
                ));
                continue;
            };
            let parent_constraints = flow
                .contexts
                .constraint_refs
                .span_or_empty(statement.entry_constraints)
                .iter()
                .enumerate()
                .filter(|(_, constraint)| {
                    constraint.kind
                        == psi_checked_trees::FlowConstraintKind::BorrowLoan { loan: *parent_loan }
                })
                .filter_map(|(offset, _)| span_handle(statement.entry_constraints, offset))
                .collect::<Vec<_>>();
            let [parent_entry_constraint] = parent_constraints.as_slice() else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow suspension requires exactly one parent entry constraint",
                ));
                continue;
            };

            resources.push(CheckedReborrowLoanResourceDraft {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: psi_checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
                    segments: borrow.loan_segments(loan).to_vec(),
                },
                access: loan.kind.clone(),
                parent_access: parent.kind.clone(),
                access_effect,
                activation_source: activation.source,
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
                parent_loan: *parent_loan,
                child_activation,
                parent_entry_constraint: *parent_entry_constraint,
                child_weakening,
                parent_weakening: *parent_weakening,
                parent_lexical_status,
            });
        }
    }

    if diagnostics.is_empty() {
        Ok(resources)
    } else {
        Err(diagnostics)
    }
}

fn parent_lexical_status_at_child_end(
    parent_source: FlowInvalidationSource,
    parent_reason: psi_checked_trees::FlowBorrowWeakeningReason,
    child_source: FlowInvalidationSource,
    child_reason: psi_checked_trees::FlowBorrowWeakeningReason,
) -> Option<ParentLexicalStatusAtChildEnd> {
    let parent = weakening_boundary_key(parent_source, parent_reason)?;
    let child = weakening_boundary_key(child_source, child_reason)?;
    Some(match parent.cmp(&child) {
        std::cmp::Ordering::Less => ParentLexicalStatusAtChildEnd::RetiredBeforeChild,
        std::cmp::Ordering::Equal => ParentLexicalStatusAtChildEnd::RetiredWithChild,
        std::cmp::Ordering::Greater => ParentLexicalStatusAtChildEnd::LivePastChild,
    })
}

fn invalid_reborrow_attenuation_diagnostic(
    parent: &psi_checked_trees::BorrowAccessKind,
    child: &psi_checked_trees::BorrowAccessKind,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot derive {} reborrow authority from an exact {} parent loan; allowed direct reborrow access pairs are Read->Read, Mutable->Read, Mutable->Mutable, Mutable->WriteOnly, and WriteOnly->WriteOnly",
        borrow_access_name(child),
        borrow_access_name(parent),
    ))
}

fn borrow_access_name(access: &psi_checked_trees::BorrowAccessKind) -> &'static str {
    match access {
        psi_checked_trees::BorrowAccessKind::Read => "Read",
        psi_checked_trees::BorrowAccessKind::Mutable => "Mutable",
        psi_checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
    }
}

fn weakening_boundary_key(
    source: FlowInvalidationSource,
    reason: psi_checked_trees::FlowBorrowWeakeningReason,
) -> Option<(usize, u8)> {
    let FlowInvalidationSource::Statement { statement_index } = source else {
        return None;
    };
    let phase = match reason {
        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired => 0,
        psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => 1,
        psi_checked_trees::FlowBorrowWeakeningReason::StateExit => 2,
    };
    Some((statement_index, phase))
}

fn span_handle<T>(span: psi_arena::HandleSpan<T>, offset: usize) -> Option<psi_arena::Handle<T>> {
    let offset = u32::try_from(offset).ok()?;
    let arena_index = span.start().arena_index().checked_add(offset)?;
    Some(psi_arena::Handle::from_parts(
        arena_index,
        span.start().generation(),
    ))
}

fn replay_checked_direct_reborrow_lineage(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
) -> Result<(), Vec<Diagnostic>> {
    for (_, state) in borrow.states.iter() {
        let Some(typed_state) = crate::semantic_calls::find_state_in_machine(
            program,
            state.machine_symbol,
            state.state_symbol,
        ) else {
            return Err(vec![Diagnostic::error(
                "checked borrow loan lineage has no exact typed state owner",
            )]);
        };
        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            let expected =
                expected_loan_lineage(program, typed_state, borrow, state, loan_handle, loan);
            if loan.lineage != expected {
                return Err(vec![Diagnostic::error(
                    "checked borrow loan lineage drifted from independent direct-reborrow replay",
                )]);
            }
        }
    }
    Ok(())
}

fn expected_loan_lineage(
    program: &psi_typed_trees::TypedTrees,
    typed_state: &psi_typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &psi_checked_trees::StateBorrowFact,
    loan_handle: psi_arena::Handle<BorrowLoanFact>,
    loan: &BorrowLoanFact,
) -> BorrowLoanLineage {
    let Some(statement) = program
        .statement_table
        .statements(typed_state.statement_nodes)
        .get(loan.statement_index)
    else {
        return if loan.source_owner_symbol.is_valid() {
            BorrowLoanLineage::UnretainedDerived
        } else {
            BorrowLoanLineage::DirectRoot
        };
    };
    let psi_checked_trees::statement::StatementNode::LocalData(local) = statement else {
        if let psi_checked_trees::statement::StatementNode::Assignment(assignment) = statement
            && matches!(
                program.expression_table.expression(assignment.value),
                psi_checked_trees::expression::ExpressionNode::Call(_)
                    | psi_checked_trees::expression::ExpressionNode::Cast(_)
                    | psi_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
                    | psi_checked_trees::expression::ExpressionNode::StructLiteral(_)
            )
        {
            return BorrowLoanLineage::UnretainedDerived;
        }
        return if loan.source_owner_symbol.is_valid() {
            BorrowLoanLineage::UnretainedDerived
        } else {
            BorrowLoanLineage::DirectRoot
        };
    };
    if local.symbol != loan.owner_symbol {
        return BorrowLoanLineage::UnretainedDerived;
    }

    match program.expression_table.expression(local.initial_value) {
        psi_checked_trees::expression::ExpressionNode::Borrow(reborrow) => {
            expected_explicit_reborrow_parent(
                program,
                typed_state,
                borrow,
                state,
                loan_handle,
                loan,
                reborrow.target,
            )
            .map(|parent_loan| BorrowLoanLineage::Reborrow { parent_loan })
            .unwrap_or_else(|| {
                if loan.source_owner_symbol.is_valid() {
                    BorrowLoanLineage::UnretainedDerived
                } else {
                    BorrowLoanLineage::DirectRoot
                }
            })
        }
        psi_checked_trees::expression::ExpressionNode::Call(_)
        | psi_checked_trees::expression::ExpressionNode::Cast(_)
        | psi_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
        | psi_checked_trees::expression::ExpressionNode::StructLiteral(_) => {
            BorrowLoanLineage::UnretainedDerived
        }
        _ if loan.source_owner_symbol.is_valid() => BorrowLoanLineage::UnretainedDerived,
        _ => BorrowLoanLineage::DirectRoot,
    }
}

#[allow(clippy::too_many_arguments)]
fn expected_explicit_reborrow_parent(
    program: &psi_typed_trees::TypedTrees,
    typed_state: &psi_typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &psi_checked_trees::StateBorrowFact,
    child_handle: psi_arena::Handle<BorrowLoanFact>,
    child: &BorrowLoanFact,
    source_expression: psi_checked_trees::expression::ExpressionHandle,
) -> Option<psi_arena::Handle<BorrowLoanFact>> {
    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        typed_state.symbol,
        child.statement_index,
        source_expression,
    )?;
    let psi_facts::PlaceRoot::Symbol(source_root) = source.root else {
        return None;
    };
    let mut candidates = borrow
        .loans
        .iter()
        .filter(|(parent_handle, parent)| {
            *parent_handle != child_handle
                && borrow.state_owns_loan(state, *parent_handle)
                && parent.statement_index < child.statement_index
                && parent.lineage != BorrowLoanLineage::UnretainedDerived
                && parent.owner_symbol == source_root
                && owner_path_matches_source(
                    program,
                    borrow.loan_owner_path(parent),
                    &source.segments,
                )
                && child.source_owner_symbol == parent.owner_symbol
        })
        .map(|(handle, parent)| (handle, parent));
    let (parent_handle, parent) = candidates.next()?;
    if candidates.next().is_some()
        || !child_place_replays_from_parent(borrow, parent, &source.segments, child)
    {
        return None;
    }
    Some(parent_handle)
}

fn child_place_replays_from_parent(
    borrow: &BorrowFacts,
    parent: &BorrowLoanFact,
    source_segments: &[psi_facts::PlaceSegment],
    child: &BorrowLoanFact,
) -> bool {
    let parent_owner_path = borrow.loan_owner_path(parent);
    let Some(remainder) = source_segments.get(parent_owner_path.len()..) else {
        return false;
    };
    child.root_symbol == parent.root_symbol
        && borrow.loan_segments(child).len() == borrow.loan_segments(parent).len() + remainder.len()
        && borrow
            .loan_segments(child)
            .iter()
            .eq(borrow.loan_segments(parent).iter().chain(remainder))
}

fn owner_path_matches_source(
    program: &psi_typed_trees::TypedTrees,
    owner_path: &[psi_checked_trees::BorrowLoanOwnerSegment],
    source_segments: &[psi_facts::PlaceSegment],
) -> bool {
    owner_path.len() <= source_segments.len()
        && owner_path
            .iter()
            .zip(source_segments)
            .all(|(owner, source)| match (owner, source) {
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Field(owner_symbol),
                    psi_facts::PlaceSegment::Field {
                        symbol: source_symbol,
                    },
                ) => !source_symbol.is_valid() || owner_symbol == source_symbol,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Case(owner_variant),
                    psi_facts::PlaceSegment::Case {
                        variant: source_variant,
                    },
                ) => owner_variant == source_variant,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::FixedIndex {
                        index: source_index,
                    },
                ) => owner_index == source_index,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|source_index| *owner_index == source_index),
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::DynamicIndex,
                    psi_facts::PlaceSegment::FixedIndex { .. }
                    | psi_facts::PlaceSegment::FixedRange { .. }
                    | psi_facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}
