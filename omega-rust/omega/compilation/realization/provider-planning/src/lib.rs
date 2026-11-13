#![forbid(unsafe_code)]

// Provider metadata and retained Terminal/native plan data. Source-checking
// producers were removed with the old checker; typed declarations enter via
// their actual surviving owner and never stand in for proved applications.

pub mod calling_policy_plans;
pub mod data_layout_queries;
pub mod dispatch_tables;
pub mod native_bindings;
pub(crate) use optimization_core::digest_encoder;
pub mod evaluated_via_bindings;
mod intrinsic_requirements;
mod sealed_accessors;
pub use sealed_accessors::{SealedAccessor, SealedAccessorScalarOperation};
pub mod program_entry;
mod provider_identity;
mod provider_instances;
pub use provider_identity::{
    provider_row_matches, provider_trait_application, provider_trait_requirement_matches,
};
pub use provider_instances::SelectedProviderSource;
pub mod intrinsic_declarations;
mod provider_planning;
mod provider_settlement;
pub mod service_declarations;
pub mod service_schema;
#[cfg(test)]
mod source_guards;
pub mod target_machines;

pub use provider_settlement::SettledTargetMachineDeclarations;
pub use provider_settlement::{
    ComponentAssumptionDemand, ProviderSettlement, verify_selected_compositions_are_realized,
};

pub use build_time_evaluation::provider_declarations::{
    ProviderSelection, ProviderSelectionSubject, TargetProviderDefaults,
};
pub use intrinsic_requirements::{
    IntrinsicRequirement, IntrinsicRequirementKind, primitive_numeric_type,
};
pub use language_semantics::declaration_selection::CompositionMode;
pub use provider_planning::provider_rows::{
    DeferredProviderRows, DirectBoundaryInput, DirectBoundaryMechanism, DirectBoundarySource,
    PreparedProviderRows, ProviderPlanCandidate, ProviderRowDeclarations,
};
pub use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation, ProviderSchemaDeclaration,
    ProviderSelectionProvenance, SelectedProviderReviewProvenance, same_semantic_name,
    satisfies_plan_name,
};
pub(crate) use provider_planning::{
    DerivedProviderPlan, ProviderPlanProvenance, SelectedTargetMachineOrigin,
};
pub use typed_trees::provider_declarations::ProviderPlanningContext;

pub use provider_planning::ProviderRowRealization;
