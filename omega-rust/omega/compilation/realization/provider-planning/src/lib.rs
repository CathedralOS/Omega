#![forbid(unsafe_code)]

//! Omega provider-plan derivation and realization.
//!
//! This crate owns provider selection, calling-policy realization, component
//! progress and task activation planning. The
//! compiler coordinator supplies checked inputs and consumes the resulting
//! plans; it does not define their domain model. Exact grants are resolved by
//! backend provider evidence, independently of compiler trust reporting.
//! Checked-tree and package-input custody dependencies remain a transitional
//! realization bridge, not a completed Terminal-only handoff: typed vocabulary
//! reaches this crate only through `checked_trees` re-exports, and
//! `source_guards` fails the build if a source-input dependency edge or a
//! facade-free typed spelling returns.
//!
//! Start at `provider_settlement.rs` for typed candidate derivation and selection,
//! then `provider_planning.rs` for checked-program binding;
//! every other folder is one public vocabulary the compiler consumes by path.
//! Source selection declarations are owned by Psi compile-time admission; the
//! re-exported selection records below are inputs, not a second source decoder.

pub mod calling_policy_plans;
pub mod component_progress;
pub mod data_layout_queries;
pub(crate) use optimization_core::digest_encoder;
pub mod evaluated_via_bindings;
mod fnv1a;
mod intrinsic_requirements;
mod sealed_placed_accessors;
pub use sealed_placed_accessors::{SealedPlacedAccessor, SealedPlacedScalarOperation};
pub mod program_entry;
mod provider_identity;
pub use provider_identity::{
    ProviderReceiverApplication, provider_receiver_application, provider_row_matches,
    provider_trait_application, provider_trait_requirement_matches,
};
mod provider_planning;
mod provider_settlement;
pub mod service_schema;
#[cfg(test)]
mod source_guards;
pub mod target_machines;
pub mod task_plans;
pub mod x86_fma_plan_association;

pub(crate) use provider_settlement::SettledTargetMachineDeclarations;
pub use provider_settlement::{
    CheckedProviderSelection, ProviderSettlement, settle_checked_providers,
    verify_selected_compositions_are_realized,
};

pub use build_time_evaluation::provider_declarations::{
    CompositionMode, ProviderPlanningContext, ProviderSelection, ProviderSelectionSubject,
    TargetProviderDefaults,
};
pub use intrinsic_requirements::{
    IntrinsicRequirement, IntrinsicRequirementKind, primitive_numeric_type,
};
pub use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation, ProviderPlanDerivation, ProviderSchemaDeclaration,
    ProviderSelectionProvenance, SelectedProviderReviewProvenance,
    bind_selected_provider_plan_facts, derive_satisfies_plans, exact_checked_adapter,
    same_semantic_name, satisfies_plan_name, select_derived_provider_plans,
    selected_provider_plan_facts, selected_requirement_checked_adapter, selected_requirement_plan,
    selected_use_plan, specialized_provider_machines,
};
pub(crate) use provider_planning::{
    DerivedProviderPlan, ProviderPlanProvenance, SelectedProviderPlanWithProvenance,
    SelectedTargetMachineOrigin, selected_provider_plan_facts_with_independent_components,
};
pub(crate) use provider_planning::{
    extract_native_external_binding_rows, validate_selected_synchronous_invocation_cycles,
};
