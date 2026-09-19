#![forbid(unsafe_code)]

//! Checked provider-plan derivation and realization.
//!
//! This crate owns provider selection, calling-policy realization, component
//! progress, task activation planning, and boundary-provider approval. The
//! compiler coordinator supplies checked inputs and consumes the resulting
//! plans; it does not define their domain model.
//!
//! Start at `provider_planning.rs` for selection and checked-program binding;
//! every other folder is one public vocabulary the compiler consumes by path.

pub mod approval;
pub mod calling_policy_plans;
pub mod compiler_intrinsics;
pub mod component_progress;
pub mod evaluated_via_bindings;
mod provider_planning;
pub mod service_schema;
pub mod task_plans;
pub mod x86_fma_plan_association;

pub use compiler_intrinsics::requirement_view::{IntrinsicRequirement, IntrinsicRequirementKind};
pub use provider_planning::selection::{
    CompositionMode, ProviderOperatorFamilyCoordinate, ProviderOperatorFamilySelection,
    ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject,
};
#[cfg(feature = "installed-writer")]
pub use provider_planning::{
    AdmittedExternalRootEntryFactHandoff, BoundExternalRootPostHandoffWriterInvocation,
    BoundExternalRootWriterExecutionError, ExternalRootPostHandoffWriterBindingError,
    SelectedExternalRootEntryFactBinding, SelectedExternalRootPostHandoffWriterPreparation,
    SelectedExternalRootProviderPlan, SelectedExternalRootWriterPreparationError,
    ValidatedWrittenBoundExternalRootPostHandoffWriterDestination,
    WrittenBoundExternalRootConsumerValidationError,
    WrittenBoundExternalRootPostHandoffWriterDestination,
    WrittenBoundExternalRootWriterRecoveryError, bind_external_root_post_handoff_writer_invocation,
    optional_selected_external_root_provider_plan, selected_external_root_entry_fact_bindings,
    selected_external_root_provider_plan, selected_external_root_provider_plan_id,
};
pub use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation, DerivedProviderPlan, ProviderPlanDerivation,
    ProviderPlanProvenance, ProviderSchemaDeclaration, ProviderSelectionProvenance,
    SelectedProviderPlanBinding, SelectedProviderPlanWithProvenance,
    SelectedProviderReviewProvenance, SelectedTargetMachineOrigin,
    bind_selected_provider_plan_facts, compiler_intrinsic_diagnostic_label,
    compiler_intrinsic_diagnostic_label_for, derive_satisfies_plans, exact_checked_adapter,
    extract_external_binding_rows, extract_native_external_binding_rows,
    intrinsic_realization_matches_operator, primitive_float_binary_intrinsic_execution_identity,
    primitive_float_binary_intrinsic_execution_identity_for,
    primitive_integer_comparison_intrinsic_execution_identity,
    primitive_integer_comparison_intrinsic_execution_identity_for, satisfied_requirement_identity,
    satisfies_plan_name, select_derived_provider_plans, select_provider_plans,
    selected_provider_plan_facts, selected_provider_plan_facts_with_independent_components,
    settle_external_binding_rows, validate_derived_provider_plan_candidates,
    validate_provider_plan_candidates, validate_selected_synchronous_invocation_cycles,
};
