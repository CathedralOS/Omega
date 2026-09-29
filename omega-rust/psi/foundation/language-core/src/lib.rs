#![forbid(unsafe_code)]

//! Target-neutral source-language vocabulary shared across Psi frontend stages.
//!
//! Start at `source_semantics.rs`, the source-semantics tables that resolution
//! and typing consult (its `receiver_binding` and `contract_result` areas own
//! the `self` receiver and reserved `result` binder spellings); `atomic`, `cast_form`, `inline_assembly` and
//! `operator_spelling` hold the orderings, cast forms, inline-assembly
//! spelling and operator spellings the stages share;
//! `data_layout_queries` names the compiler-owned `size_of`/`align_of`
//! call spellings. The crate
//! defines vocabulary that several stages must agree on; it neither parses nor
//! judges a program, and it depends on no representation crate.

pub mod atomic;
pub mod cast_form;
pub mod data_layout_queries;
pub mod inline_assembly;
pub mod operator_spelling;
mod source_semantics;

pub use atomic::{AtomicOrderingPlan, MemoryOrdering};
pub use cast_form::CastForm;
pub use operator_spelling::OperatorSpelling;
pub use source_semantics::contract_result::{
    self, RESERVED_RESULT, is_reserved_result, signature_reserves_result,
};
pub use source_semantics::receiver_binding::{
    self, PLACE_MEMBER_SEPARATOR, SELF_RECEIVER, is_receiver_rooted, is_self_receiver,
    receiver_place_field, receiver_place_label,
};
pub use source_semantics::{
    BindingRelevance, CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin,
    CarryAddress, CarryCpu, CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension,
    DataSupplyMode, DomainClassification, DomainPredicateBody, Multiplicity, ReferenceAccess,
};
