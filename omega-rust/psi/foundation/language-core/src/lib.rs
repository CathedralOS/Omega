#![forbid(unsafe_code)]

//! Target-neutral source-language vocabulary shared across Psi frontend stages.
//!
//! Atomic orderings, cast forms, inline-assembly spelling, operator spellings,
//! and the source-semantics tables that resolution and typing consult. The crate
//! defines vocabulary that several stages must agree on; it neither parses nor
//! judges a program, and it depends on no representation crate.

pub mod atomic;
pub mod cast_form;
pub mod inline_assembly;
pub mod operator_spelling;
mod source_semantics;

pub use atomic::{AtomicOrderingPlan, MemoryOrdering};
pub use cast_form::CastForm;
pub use operator_spelling::OperatorSpelling;
pub use source_semantics::{
    BindingRelevance, CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin,
    CarryAddress, CarryCpu, CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension,
    DataSupplyMode, DomainClassification, DomainPredicateBody, Multiplicity, ReferenceAccess,
};
