#![forbid(unsafe_code)]

//! Target-neutral source-language vocabulary shared across Psi frontend stages.
//!
//! Start at `source_semantics.rs`, the source-semantics tables that resolution
//! and typing consult (its `receiver_binding`, `place_root` and
//! `contract_result` areas own the `self` receiver keyword, rendered place
//! roots, and the reserved `result` binder spelling); `atomic`, `cast_form`, `inline_assembly` and
//! `operator_spelling` hold the orderings, cast forms, inline-assembly
//! spelling and operator spellings the stages share;
//! `data_layout_queries` names the compiler-owned `size_of`/`align_of`
//! call spellings, and `constant_value` separates evaluated meaning from encoded
//! declaration values. The crate
//! defines vocabulary that several stages must agree on; it neither parses nor
//! judges a program, and it depends on no representation crate.

pub mod atomic;
pub mod cast_form;
pub mod constant_value;
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
pub use source_semantics::place_root::{
    self, PLACE_MEMBER_SEPARATOR, is_rooted_at, place_field, place_label,
};
pub use source_semantics::receiver_binding::{self, SELF_RECEIVER, is_self_receiver};
pub use source_semantics::{
    BindingRelevance, CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin,
    CarryAddress, CarryCpu, CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension,
    DataSupplyMode, DomainClassification, DomainPredicateBody, Multiplicity, ReferenceAccess,
};
