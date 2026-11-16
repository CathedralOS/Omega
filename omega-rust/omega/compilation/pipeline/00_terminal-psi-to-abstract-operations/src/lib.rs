#![forbid(unsafe_code)]

/*
  Check the selected code before lowering it, so unused library functions do
  not demand unavailable target operations. Omega 00 lowers those functions,
  establishes access to their inputs, and admits their native realization.

  The shared `verified-optimization-input` crate supplies the complete checked native proposal
  and its execution scope. This stage retains source-free native inputs through
  `retained_terminal` and its codec, lowers canonical Psi with `lowering`, and
  passes the resulting plan and selections to subsequent stages. Loose bytes
  enter through `artifact_admission`; an existing canonical artifact is reused.
*/
mod artifact_admission;
mod lowering;
mod native_execution_scope;
use ::provider_admission as providers;
pub use ::provider_admission::AdmittedInstalledEntryExecution;
pub use ::provider_admission::provider_admission;
pub(crate) use native_execution_scope::native_admission;
mod retained_terminal;
pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use native_admission::{
    AdmittedNativeRealization, NativeAdmissionRequest, admit_native_realization,
    admit_prepared_native_realization, artifact_lowering_error,
};
pub use providers::provider_admission::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    InstalledInterruptNativeEntryProposal, NativeCallbackArgumentProposal, SettledCompilerBuiltin,
};
pub use retained_terminal::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    PreparedNativeExecution, PreparedRetainedNativeProviders, RetainedComponentNativeInputs,
    RetainedNativeProviderInputs, RetainedTerminalArtifact, TerminalCallbackOccurrenceProposal,
    TerminalCallbackThunkArtifact, TerminalCompilerBuiltinProposal,
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
    TerminalNativeBoundaryApplications, TerminalNativeOccurrences, TerminalNativeProposalRequest,
    TerminalNativeProviders, TerminalNativeRealizationProposal, TerminalTargetProviderProposal,
    decode_installation_assumptions, encode_installation_assumptions, lower_native_callable,
    lower_native_execution, prepare_native_providers, project_terminal_native_realization_proposal,
    recover_native_derivation_inputs, retain_native_derivation_inputs,
};

pub(crate) use artifact_admission::ArtifactLoweringError;
pub use artifact_admission::{
    NativeExecutionSelection, NativeScalarApplications, TerminalAccessViewEstablishment,
    VerifiedNativeArtifactInput, lower_artifact,
};
pub use lowering::LoweringError;
pub use providers::provider_installation::SelectedProviderAdapter;
pub use providers::provider_installation::{
    AdmittedProviderInstallation, ProviderInstallationError,
};
pub use verified_optimization_input::callback_settlement::settle_native_callback_placements;
pub use verified_optimization_input::entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError, SelectedProgramEntryCall,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use verified_optimization_input::settlement::boundary_application_settlement::BoundaryApplicationSettlement;
pub use verified_optimization_input::{
    AdmittedLoweringFacts, NativeExecutionScope, ObligationEvidence,
    VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput,
};
