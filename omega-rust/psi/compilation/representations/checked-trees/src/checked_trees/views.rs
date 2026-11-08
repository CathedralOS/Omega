//! Field-by-field views of `CheckFacts` that the per-state permission walk
//! works through, so that what the walk may write is a compile-time fact.
//!
//! The walk in `prepare_permission_events` writes only the flow ownership
//! arenas, nine content rows and, after each state, that state's control row.
//! `CheckFactsWalk` lends exactly the ownership arenas and the nine rows
//! `&mut` and every other field `&`, under the same field paths as
//! `CheckFacts`, so helper bodies read as before. `CheckFactsRef` is the
//! all-shared view that read-only helpers take; it comes from a whole
//! `CheckFacts` (`facts.view()`) or from a walk in progress (`walk.view()`), so
//! one helper serves both. The control row stays shared here because the loop
//! applies each state's storage-transfer span between states.
//!
//! The views carry only the fields the walk and its helpers read; a helper
//! that needs another one adds it here, shared unless the walk must write it.
//! Keeping the writable set this small lets a later change give each machine
//! group its own output without cloning `CheckFacts`.

use crate::{CheckFacts, ContentProjectionFacts, FlowFacts, QualificationFacts};

/// A shared view of [`CheckFacts`], each field borrowed on its own; see the module
/// comment.
#[derive(Debug, Clone, Copy)]
pub struct CheckFactsRef<'f> {
    pub semantic: &'f facts::FactPlan,
    pub borrow: &'f crate::BorrowFacts,
    pub contextual_cleanup: &'f crate::ContextualCleanupFacts,
    pub proof: &'f crate::ProofFacts,
    pub operators: &'f crate::CheckedOperatorFacts,
    pub flow: FlowFactsRef<'f>,
    pub termination: &'f crate::TerminationFacts,
    pub qualifications: QualificationFactsRef<'f>,
}

/// [`CheckFacts`] during the permission walk: `&mut` to what the walk writes,
/// `&` to the rest.
#[derive(Debug)]
pub struct CheckFactsWalk<'f> {
    pub semantic: &'f facts::FactPlan,
    pub borrow: &'f crate::BorrowFacts,
    pub contextual_cleanup: &'f crate::ContextualCleanupFacts,
    pub proof: &'f crate::ProofFacts,
    pub operators: &'f crate::CheckedOperatorFacts,
    pub flow: FlowFactsWalk<'f>,
    pub termination: &'f crate::TerminationFacts,
    pub qualifications: QualificationFactsWalk<'f>,
}

impl CheckFacts {
    pub fn view(&self) -> CheckFactsRef<'_> {
        CheckFactsRef {
            semantic: &self.semantic,
            borrow: &self.borrow,
            contextual_cleanup: &self.contextual_cleanup,
            proof: &self.proof,
            operators: &self.operators,
            flow: self.flow.view(),
            termination: &self.termination,
            qualifications: self.qualifications.view(),
        }
    }

    /// Lend what the permission walk writes mutably and the rest shared.
    pub fn walk(&mut self) -> CheckFactsWalk<'_> {
        let Self {
            semantic,
            borrow,
            contextual_cleanup,
            proof,
            operators,
            flow,
            termination,
            qualifications,
            ..
        } = self;
        CheckFactsWalk {
            semantic,
            borrow,
            contextual_cleanup,
            proof,
            operators,
            flow: flow.walk(),
            termination,
            qualifications: qualifications.walk(),
        }
    }
}

impl CheckFactsWalk<'_> {
    pub fn view(&self) -> CheckFactsRef<'_> {
        CheckFactsRef {
            semantic: self.semantic,
            borrow: self.borrow,
            contextual_cleanup: self.contextual_cleanup,
            proof: self.proof,
            operators: self.operators,
            flow: self.flow.view(),
            termination: self.termination,
            qualifications: self.qualifications.view(),
        }
    }
}

/// A shared view of [`FlowFacts`], each field borrowed on its own; see the module
/// comment.
#[derive(Debug, Clone, Copy)]
pub struct FlowFactsRef<'f> {
    pub contexts: &'f crate::FlowContextFacts,
    pub invalidations: &'f crate::FlowInvalidationFacts,
    pub borrow_lifetimes: &'f crate::FlowBorrowLifetimeFacts,
    pub ownership: &'f crate::FlowOwnershipFacts,
    pub control: &'f crate::FlowControlFacts,
}

/// [`FlowFacts`] during the permission walk: `&mut` to what the walk writes,
/// `&` to the rest.
#[derive(Debug)]
pub struct FlowFactsWalk<'f> {
    pub contexts: &'f crate::FlowContextFacts,
    pub invalidations: &'f crate::FlowInvalidationFacts,
    pub borrow_lifetimes: &'f crate::FlowBorrowLifetimeFacts,
    pub ownership: &'f mut crate::FlowOwnershipFacts,
    pub control: &'f crate::FlowControlFacts,
}

impl FlowFacts {
    pub fn view(&self) -> FlowFactsRef<'_> {
        FlowFactsRef {
            contexts: &self.contexts,
            invalidations: &self.invalidations,
            borrow_lifetimes: &self.borrow_lifetimes,
            ownership: &self.ownership,
            control: &self.control,
        }
    }

    /// Lend what the permission walk writes mutably and the rest shared.
    pub fn walk(&mut self) -> FlowFactsWalk<'_> {
        let Self {
            contexts,
            invalidations,
            borrow_lifetimes,
            ownership,
            control,
            ..
        } = self;
        FlowFactsWalk {
            contexts,
            invalidations,
            borrow_lifetimes,
            ownership,
            control,
        }
    }
}

impl FlowFactsWalk<'_> {
    pub fn view(&self) -> FlowFactsRef<'_> {
        FlowFactsRef {
            contexts: self.contexts,
            invalidations: self.invalidations,
            borrow_lifetimes: self.borrow_lifetimes,
            ownership: &*self.ownership,
            control: self.control,
        }
    }
}

/// A shared view of [`QualificationFacts`], each field borrowed on its own; see the module
/// comment.
#[derive(Debug, Clone, Copy)]
pub struct QualificationFactsRef<'f> {
    pub content: ContentProjectionFactsRef<'f>,
}

/// [`QualificationFacts`] during the permission walk: `&mut` to what the walk writes,
/// `&` to the rest.
#[derive(Debug)]
pub struct QualificationFactsWalk<'f> {
    pub content: ContentProjectionFactsWalk<'f>,
}

impl QualificationFacts {
    pub fn view(&self) -> QualificationFactsRef<'_> {
        QualificationFactsRef {
            content: self.content.view(),
        }
    }

    /// Lend what the permission walk writes mutably and the rest shared.
    pub fn walk(&mut self) -> QualificationFactsWalk<'_> {
        let Self { content, .. } = self;
        QualificationFactsWalk {
            content: content.walk(),
        }
    }
}

impl QualificationFactsWalk<'_> {
    pub fn view(&self) -> QualificationFactsRef<'_> {
        QualificationFactsRef {
            content: self.content.view(),
        }
    }
}

/// A shared view of [`ContentProjectionFacts`], each field borrowed on its own; see the module
/// comment.
#[derive(Debug, Clone, Copy)]
pub struct ContentProjectionFactsRef<'f> {
    pub plans: &'f Vec<language_semantics::content::ContentProjectionPlan>,
    pub conservation_plans: &'f Vec<language_semantics::content::ContentConservationPlan>,
    pub body_conservations: &'f arena::Arena<super::facts::ContentBodyConservationFact>,
    pub body_conservation_plans: &'f Vec<language_semantics::content::ContentConservationPlan>,
    pub body_claim_bindings: &'f arena::Arena<super::facts::ContentBodyClaimBinding>,
    pub body_available_inputs: &'f arena::Arena<super::facts::ContentBodyAvailableInput>,
    pub body_transformations: &'f arena::Arena<super::facts::ContentBodyTransformation>,
    pub body_derivations: &'f arena::Arena<super::facts::ContentBodyDerivation>,
    pub boundary_requirement_plans: &'f Vec<language_semantics::content::ContentConservationPlan>,
    pub boundary_body_requirements: &'f arena::Arena<super::facts::ContentBoundaryBodyRequirement>,
    pub backing_addresses: &'f arena::Arena<super::facts::ContentBackingAddressTransport>,
    pub claim_transformations: &'f arena::Arena<super::facts::ContentClaimTransformation>,
}

/// [`ContentProjectionFacts`] during the permission walk: `&mut` to what the walk writes,
/// `&` to the rest.
#[derive(Debug)]
pub struct ContentProjectionFactsWalk<'f> {
    pub plans: &'f Vec<language_semantics::content::ContentProjectionPlan>,
    pub conservation_plans: &'f Vec<language_semantics::content::ContentConservationPlan>,
    pub body_conservations: &'f mut arena::Arena<super::facts::ContentBodyConservationFact>,
    pub body_conservation_plans: &'f mut Vec<language_semantics::content::ContentConservationPlan>,
    pub body_claim_bindings: &'f mut arena::Arena<super::facts::ContentBodyClaimBinding>,
    pub body_available_inputs: &'f mut arena::Arena<super::facts::ContentBodyAvailableInput>,
    pub body_transformations: &'f mut arena::Arena<super::facts::ContentBodyTransformation>,
    pub body_derivations: &'f mut arena::Arena<super::facts::ContentBodyDerivation>,
    pub boundary_requirement_plans:
        &'f mut Vec<language_semantics::content::ContentConservationPlan>,
    pub boundary_body_requirements:
        &'f mut arena::Arena<super::facts::ContentBoundaryBodyRequirement>,
    pub backing_addresses: &'f mut arena::Arena<super::facts::ContentBackingAddressTransport>,
    pub claim_transformations: &'f mut arena::Arena<super::facts::ContentClaimTransformation>,
}

impl ContentProjectionFacts {
    pub fn view(&self) -> ContentProjectionFactsRef<'_> {
        ContentProjectionFactsRef {
            plans: &self.plans,
            conservation_plans: &self.conservation_plans,
            body_conservations: &self.body_conservations,
            body_conservation_plans: &self.body_conservation_plans,
            body_claim_bindings: &self.body_claim_bindings,
            body_available_inputs: &self.body_available_inputs,
            body_transformations: &self.body_transformations,
            body_derivations: &self.body_derivations,
            boundary_requirement_plans: &self.boundary_requirement_plans,
            boundary_body_requirements: &self.boundary_body_requirements,
            backing_addresses: &self.backing_addresses,
            claim_transformations: &self.claim_transformations,
        }
    }

    /// Lend what the permission walk writes mutably and the rest shared.
    pub fn walk(&mut self) -> ContentProjectionFactsWalk<'_> {
        let Self {
            plans,
            conservation_plans,
            body_conservations,
            body_conservation_plans,
            body_claim_bindings,
            body_available_inputs,
            body_transformations,
            body_derivations,
            boundary_requirement_plans,
            boundary_body_requirements,
            backing_addresses,
            claim_transformations,
            ..
        } = self;
        ContentProjectionFactsWalk {
            plans,
            conservation_plans,
            body_conservations,
            body_conservation_plans,
            body_claim_bindings,
            body_available_inputs,
            body_transformations,
            body_derivations,
            boundary_requirement_plans,
            boundary_body_requirements,
            backing_addresses,
            claim_transformations,
        }
    }
}

impl ContentProjectionFactsWalk<'_> {
    pub fn view(&self) -> ContentProjectionFactsRef<'_> {
        ContentProjectionFactsRef {
            plans: self.plans,
            conservation_plans: self.conservation_plans,
            body_conservations: &*self.body_conservations,
            body_conservation_plans: &*self.body_conservation_plans,
            body_claim_bindings: &*self.body_claim_bindings,
            body_available_inputs: &*self.body_available_inputs,
            body_transformations: &*self.body_transformations,
            body_derivations: &*self.body_derivations,
            boundary_requirement_plans: &*self.boundary_requirement_plans,
            boundary_body_requirements: &*self.boundary_body_requirements,
            backing_addresses: &*self.backing_addresses,
            claim_transformations: &*self.claim_transformations,
        }
    }
}
