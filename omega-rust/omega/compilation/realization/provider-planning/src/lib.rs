#![forbid(unsafe_code)]

//! Omega provider-plan derivation and realization.
//!
//! This crate owns provider selection, calling-policy realization, component
//! progress and task activation planning. The
//! compiler coordinator supplies checked inputs and consumes the resulting
//! plans; it does not define their domain model. Exact grants are resolved by
//! backend provider evidence, independently of compiler trust reporting.
//! Checked-tree and package-input custody dependencies remain a transitional
//! realization bridge, not a completed Terminal-only handoff.
//!
//! Start at `provider_settlement.rs` for typed candidate derivation and selection,
//! then `provider_planning.rs` for checked-program binding;
//! every other folder is one public vocabulary the compiler consumes by path.

pub mod calling_policy_plans;
pub mod component_progress;
pub mod data_layout_queries;
pub mod evaluated_via_bindings;
mod intrinsic_requirements;
mod planning_context;
pub mod program_entry;
mod provider_declarations;
mod provider_planning;
mod provider_settlement;
pub mod service_schema;
pub mod target_machines;
pub mod task_plans;
pub mod x86_fma_plan_association;

pub use provider_settlement::{
    CheckedProviderSelection, MintedFilesystemHostPlan, MintedTimeHostPlan,
    SettledTargetMachineDeclarations, mint_canonical_filesystem_host_plan,
    mint_canonical_time_host_plan, settle_checked_providers,
    verify_independent_component_descriptions, verify_selected_compositions_are_realized,
};

pub use intrinsic_requirements::{IntrinsicRequirement, IntrinsicRequirementKind};
pub use planning_context::ProviderPlanningContext;
pub use provider_declarations::{
    collect_executed_provider_selections, harvest_provider_selections,
    validate_executed_provider_selection_declaration,
};
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
    bind_selected_provider_plan_facts, derive_satisfies_plans, exact_checked_adapter,
    extract_external_binding_rows, extract_native_external_binding_rows,
    satisfied_requirement_identity, satisfies_plan_name, select_derived_provider_plans,
    select_provider_plans, selected_provider_plan_facts,
    selected_provider_plan_facts_with_independent_components, selected_requirement_checked_adapter,
    selected_requirement_plan, selected_use_plan, settle_external_binding_rows,
    validate_derived_provider_plan_candidates, validate_provider_plan_candidates,
    validate_selected_synchronous_invocation_cycles,
};
