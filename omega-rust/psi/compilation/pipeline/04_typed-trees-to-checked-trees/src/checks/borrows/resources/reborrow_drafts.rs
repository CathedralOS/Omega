//! Draft rows for reborrow loan resources, disposition events, containment
//! certificates and restored call uses.

use checked_trees::{
    BorrowFacts, BorrowLoanFact, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
    CheckedParentBorrowResource, CheckedReborrowAccessEffect,
    CheckedReborrowContainmentCertificate, CheckedReborrowContainmentKind,
    CheckedReborrowLoanResource, CheckedReborrowParentEndStatus,
    CheckedReborrowParentSuspensionBoundary, CheckedReborrowResourceDisposition,
    CheckedReborrowResourceDispositionEvent, CheckedReborrowRestorationObligation,
    CheckedReborrowRestoredCallUseCertificate, CheckedRetiredParentResourceDispositionStep,
    FlowInvalidationSource, ParentLexicalStatusAtChildEnd,
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
    pub(crate) fn close(
        &self,
        parent_resource: CheckedParentBorrowResource,
    ) -> CheckedReborrowLoanResource {
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
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResourceHandles {
    pub(crate) direct: Vec<arena::Handle<CheckedDirectBorrowLoanResource>>,
    pub(crate) reborrows: Vec<arena::Handle<CheckedReborrowLoanResource>>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedReborrowContainmentCertificateDraft {
    pub(crate) machine_symbol: symbols::SymbolHandle,
    pub(crate) state_symbol: symbols::SymbolHandle,
    pub(crate) child_loan: arena::Handle<BorrowLoanFact>,
    pub(crate) child_resource: usize,
    pub(crate) parent_loan: arena::Handle<BorrowLoanFact>,
    pub(crate) parent_resource: ParentResourceIndex,
    pub(crate) parent_access: checked_trees::BorrowAccessKind,
    pub(crate) child_access: checked_trees::BorrowAccessKind,
    pub(crate) access_effect: CheckedReborrowAccessEffect,
    pub(crate) child_activation: arena::Handle<checked_trees::FlowBorrowActivationFact>,
    pub(crate) parent_entry_constraint: arena::Handle<checked_trees::FlowConstraintRef>,
    pub(crate) formation_source: FlowInvalidationSource,
    pub(crate) child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    pub(crate) parent_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    pub(crate) child_weakening_source: FlowInvalidationSource,
    pub(crate) child_weakening_reason: checked_trees::FlowBorrowWeakeningReason,
    pub(crate) parent_place: checked_trees::CapturedPlace,
    pub(crate) child_place: checked_trees::CapturedPlace,
    pub(crate) projection_remainder: Vec<facts::PlaceSegment>,
    pub(crate) containment: CheckedReborrowContainmentKind,
}

impl CheckedReborrowContainmentCertificateDraft {
    pub(crate) fn close(&self, handles: &ResourceHandles) -> CheckedReborrowContainmentCertificate {
        CheckedReborrowContainmentCertificate {
            machine_symbol: self.machine_symbol,
            state_symbol: self.state_symbol,
            child_loan: self.child_loan,
            child_resource: handles.reborrows[self.child_resource],
            parent_loan: self.parent_loan,
            parent_resource: handles.parent(self.parent_resource),
            parent_access: self.parent_access.clone(),
            child_access: self.child_access.clone(),
            access_effect: self.access_effect,
            child_activation: self.child_activation,
            parent_entry_constraint: self.parent_entry_constraint,
            formation_source: self.formation_source,
            child_weakening: self.child_weakening,
            parent_weakening: self.parent_weakening,
            child_weakening_source: self.child_weakening_source,
            child_weakening_reason: self.child_weakening_reason,
            parent_place: self.parent_place.clone(),
            child_place: self.child_place.clone(),
            projection_remainder: self.projection_remainder.clone(),
            containment: self.containment,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedReborrowRestoredCallUseCertificateDraft {
    pub(crate) machine_symbol: symbols::SymbolHandle,
    pub(crate) state_symbol: symbols::SymbolHandle,
    pub(crate) child_loan: arena::Handle<BorrowLoanFact>,
    pub(crate) child_resource: usize,
    pub(crate) parent_loan: arena::Handle<BorrowLoanFact>,
    pub(crate) parent_resource: usize,
    pub(crate) disposition: usize,
    pub(crate) containment: usize,
    pub(crate) child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    pub(crate) call: arena::Handle<checked_trees::FlowCallFact>,
    pub(crate) borrow_call: arena::Handle<checked_trees::BorrowCallFact>,
    pub(crate) call_access: arena::Handle<checked_trees::BorrowArgumentAccessFact>,
    pub(crate) parent_entry_constraint: arena::Handle<checked_trees::FlowConstraintRef>,
    pub(crate) carrier_place: checked_trees::CapturedPlace,
    pub(crate) restored_place: checked_trees::CapturedPlace,
    pub(crate) access: checked_trees::BorrowAccessKind,
    pub(crate) target_symbol: symbols::SymbolHandle,
}

impl CheckedReborrowRestoredCallUseCertificateDraft {
    pub(crate) fn close(
        &self,
        resources: &ResourceHandles,
        dispositions: &[arena::Handle<CheckedReborrowResourceDispositionEvent>],
        containments: &[arena::Handle<CheckedReborrowContainmentCertificate>],
    ) -> CheckedReborrowRestoredCallUseCertificate {
        CheckedReborrowRestoredCallUseCertificate {
            machine_symbol: self.machine_symbol,
            state_symbol: self.state_symbol,
            child_loan: self.child_loan,
            child_resource: resources.reborrows[self.child_resource],
            parent_loan: self.parent_loan,
            parent_resource: resources.direct[self.parent_resource],
            disposition: dispositions[self.disposition],
            containment: containments[self.containment],
            child_weakening: self.child_weakening,
            call: self.call,
            borrow_call: self.borrow_call,
            call_access: self.call_access,
            parent_entry_constraint: self.parent_entry_constraint,
            carrier_place: self.carrier_place.clone(),
            restored_place: self.restored_place.clone(),
            access: self.access.clone(),
            target_symbol: self.target_symbol,
        }
    }
}
