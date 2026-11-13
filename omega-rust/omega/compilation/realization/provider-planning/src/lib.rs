#![forbid(unsafe_code)]

// Canonical Terminal/native plan data and Build-row receipt normalization.
// Source inspection and nominations belong to the source coordinator; missing
// provider authority never becomes a source fallback or a fabricated proof.

pub mod calling_policy_plans;
pub mod native_bindings;
pub(crate) use optimization_core::digest_encoder;
pub mod evaluated_via_bindings;
pub mod program_entry;
mod provider_planning;
mod provider_settlement;
pub mod service_declarations;
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
pub use language_semantics::declaration_selection::CompositionMode;
pub use provider_planning::provider_rows::{
    DirectBoundaryInput, DirectBoundaryMechanism, DirectBoundarySource, ProviderPlanCandidate,
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

pub use provider_planning::ProviderRowRealization;
