//! Draft rows for reborrow loan resources and disposition events.

use checked_trees::{
    BorrowFacts, BorrowLoanFact, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
    CheckedParentBorrowResource as BorrowResource, CheckedReborrowAccessEffect,
    CheckedReborrowLoanResource as LoanResource, CheckedReborrowParentEndStatus,
    CheckedReborrowParentSuspensionBoundary, CheckedReborrowResourceDisposition,
    CheckedReborrowResourceDispositionEvent, CheckedReborrowRestorationObligation,
    CheckedRetiredParentResourceDispositionStep, FlowInvalidationSource,
    ParentLexicalStatusAtChildEnd,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedReborrowLoanResourceDraft {
    pub(crate) loan: arena::Handle<BorrowLoanFact>,
    pub(crate) machine_symbol: symbols::SymbolHandle,
    pub(crate) state_symbol: symbols::SymbolHandle,
    pub(crate) owner_symbol: symbols::SymbolHandle,
    pub(crate) owner_path: Vec<checked_trees::BorrowLoanOwnerSegment>,
    pub(crate) captured_place: checked_trees::CapturedPlace,
    pub(crate) access: checked_trees::BorrowAccessKind,
    pub(crate) parent_access: checked_trees::BorrowAccessKind,
    pub(crate) access_effect: CheckedReborrowAccessEffect,
    pub(crate) activation_source: checked_trees::FlowInvalidationSource,
    pub(crate) weakening_source: checked_trees::FlowInvalidationSource,
    pub(crate) weakening_reason: checked_trees::FlowBorrowWeakeningReason,
    pub(crate) parent_loan: arena::Handle<BorrowLoanFact>,
    pub(crate) child_activation: arena::Handle<checked_trees::FlowBorrowActivationFact>,
    pub(crate) parent_entry_constraint: arena::Handle<checked_trees::FlowConstraintRef>,
    pub(crate) child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    pub(crate) parent_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    pub(crate) parent_lexical_status: ParentLexicalStatusAtChildEnd,
}

impl CheckedReborrowLoanResourceDraft {
    pub(crate) fn close(&self, parent_resource: BorrowResource) -> LoanResource {
        LoanResource {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParentResourceIndex {
    Direct(usize),
    Reborrow(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DispositionTargetIndex {
    ParentResource(ParentResourceIndex),
    DirectRootLifetime(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedReborrowDispositionEventDraft {
    pub(crate) machine_symbol: symbols::SymbolHandle,
    pub(crate) state_symbol: symbols::SymbolHandle,
    pub(crate) child_loan: arena::Handle<BorrowLoanFact>,
    pub(crate) child_resource: usize,
    pub(crate) child_activation: arena::Handle<checked_trees::FlowBorrowActivationFact>,
    pub(crate) child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    pub(crate) parent_loan: arena::Handle<BorrowLoanFact>,
    pub(crate) parent_resource: ParentResourceIndex,
    pub(crate) boundary_source: FlowInvalidationSource,
    pub(crate) boundary_phase: CheckedBorrowResourceLifecyclePhase,
    pub(crate) shared_cohort: Vec<usize>,
    pub(crate) retired_parent_path: Vec<(
        ParentResourceIndex,
        arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    )>,
    pub(crate) final_target: DispositionTargetIndex,
    pub(crate) disposition: CheckedReborrowResourceDisposition,
    /// Exact post-restoration mutating-call join; the three stay absent unless
    /// `bind_restored_call_uses` certifies this event's boundary call.
    pub(crate) restored_call: arena::Handle<checked_trees::FlowCallFact>,
    pub(crate) restored_borrow_call: arena::Handle<checked_trees::BorrowCallFact>,
    pub(crate) restored_call_access: arena::Handle<checked_trees::BorrowArgumentAccessFact>,
}

impl CheckedReborrowDispositionEventDraft {
    pub(crate) fn close(
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
            restored_call: self.restored_call,
            restored_borrow_call: self.restored_borrow_call,
            restored_call_access: self.restored_call_access,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResourceHandles {
    pub(crate) direct: Vec<arena::Handle<CheckedDirectBorrowLoanResource>>,
    pub(crate) reborrows: Vec<arena::Handle<LoanResource>>,
}

impl ResourceHandles {
    fn parent(&self, index: ParentResourceIndex) -> BorrowResource {
        match index {
            ParentResourceIndex::Direct(index) => BorrowResource::DirectRoot {
                resource: self.direct[index],
            },
            ParentResourceIndex::Reborrow(index) => BorrowResource::Reborrow {
                resource: self.reborrows[index],
            },
        }
    }
}
