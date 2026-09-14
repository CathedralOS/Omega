#![forbid(unsafe_code)]

//! Checked provider-plan derivation and realization.
//!
//! This crate owns provider selection, calling-policy realization, component
//! progress, task activation planning, and boundary-provider approval. The
//! compiler coordinator supplies checked inputs and consumes the resulting
//! plans; it does not define their domain model.
//!
//! Start in `provider_planning.rs` for selection and checked-program binding.

pub mod approval;
pub mod calling_policy_plans;
pub mod component_progress;
pub mod evaluated_via_bindings;
mod provider_planning;
mod selection;
pub mod service_schema;
pub mod task_plans;
pub mod x86_fma_plan_association;

pub use provider_planning::*;
pub use selection::{
    CompositionMode, ProviderOperatorFamilyCoordinate, ProviderOperatorFamilySelection,
    ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject,
};
