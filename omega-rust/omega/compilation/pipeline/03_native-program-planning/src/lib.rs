//! Omega 00 settles the target, calling plans and installation reach of an accepted program.
#![forbid(unsafe_code)]

// Canonical Terminal/native plan data and Build-row receipt normalization.
// Source inspection and nominations belong to the source coordinator; missing
// provider authority never becomes a source fallback or a fabricated proof.

pub mod calling_policy_plans;
pub use backend_plan::{compiler_primitive_schemas, hosted_requirement_schema};
pub mod native_bindings;
pub(crate) use ::wire::digest_encoder;
pub mod program_entry;
mod provider_planning;
pub mod target_supply;

pub use declaration_contracts::selection::CompositionMode;
pub use provider_planning::provider_rows::{
    DirectBoundaryInput, DirectBoundaryMechanism, direct_compiler_primitive_leaves,
};
pub use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};

pub use terminal_semantics::logical_providers::satisfies_plan_name;

mod execution;
mod provider_execution_inputs;
pub use backend_plan::ProviderExecutionOrigins;
pub use provider_execution_inputs::plan_provider_execution_inputs;
mod package_review;
mod selected_execution;
mod folded_entry;
pub use folded_entry::FoldedTargetProgram;
pub use provider_correspondence::trust;
pub use execution::{NativeProgramPlanningError, plan_published_target_execution, plan_target_execution, plan_unscoped_execution};
pub use selected_execution::SelectedExecution;

#[cfg(feature = "test-support")]
pub mod test_support {
    use super::{SelectedExecution, target_supply::RealizedTargetSupply};
    pub use crate::abstract_optimization::build_verified_psi_optimization_unit;
    pub fn select_realization(
        profile: Option<target::TargetProfile>,
        realization: RealizedTargetSupply,
    ) -> Result<SelectedExecution, Vec<diagnostics::Diagnostic>> {
        SelectedExecution::from_realization(profile, realization)
    }
}

mod abstract_optimization;
mod artifact_lowering;
mod body_planning;
mod emission_facts;
mod preparation;
mod target_lowering;
pub use artifact_lowering::{
    AdmittedNativeRealization, NativeAdmissionRequest, NativeExecutionSelection,
    NativeScalarApplications, VerifiedNativeArtifactInput, admit_executable_entry,
    admit_native_realization, admit_prepared_native_realization, lower_artifact,
    lower_native_callable, lower_native_execution, prepare_native_providers,
};
pub use body_planning::{
    NativeBodyObservation, NativeBodyPlan, NativeBodyPlanningError, plan_native_body, plan_runtime_storage,
};
pub use target_lowering::{OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations};
pub mod replay {
    pub use crate::preparation::{
        ExecutionPlanReplayInputs, TargetOperationAuditError, verify_execution_plan,
        verify_target_operation_plan_with_closed_module,
    };
    pub use crate::target_lowering::capture_runtime_borrowed_storage_layouts;
}

pub use artifact_lowering::{
    AdmittedNativeComponentCall, BoundaryApplicationSettlement, ComponentExportInput,
    ComponentNativeInputs, NativeProgramEntrySettlement, PreparedNativeExecution,
    RetainedNativeProviderInputs, RetainedTerminalArtifact, TerminalNativeProposalRequest,
    TerminalNativeRealizationProposal, ValidatedNativeProgramEntrySettlement,
    project_terminal_native_realization_proposal,
};
pub use provider_admission::{
    AdmittedBoundarySettlement, AdmittedInstalledEntryExecution, AdmittedNativeCallbackArgument,
    AdmittedProviderInstallation, NativeCallbackArgumentProposal, SettledCompilerBuiltin,
};

pub mod observations;
pub mod callable_planning;
