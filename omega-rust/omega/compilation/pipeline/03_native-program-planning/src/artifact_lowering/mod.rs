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
mod entry_receiver;
pub use entry_receiver::admit_executable_entry;
mod lowering;
mod native_execution_scope;
use ::provider_admission as providers;
pub(crate) use native_execution_scope::native_admission;
mod retained_terminal;
pub use native_admission::{
    AdmittedNativeRealization, NativeAdmissionRequest, admit_native_realization,
    admit_prepared_native_realization,
};
pub use providers::provider_admission::NativeCallbackArgumentProposal;
pub use retained_terminal::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    PreparedNativeExecution,
    RetainedNativeProviderInputs, RetainedTerminalArtifact, TerminalNativeProposalRequest, TerminalNativeRealizationProposal,
    lower_native_callable, lower_native_execution, prepare_native_providers,
    project_terminal_native_realization_proposal,
};

pub(crate) use artifact_admission::ArtifactLoweringError;
pub use artifact_admission::{
    NativeExecutionSelection, NativeScalarApplications, TerminalAccessViewEstablishment,
    VerifiedNativeArtifactInput, lower_artifact,
};
pub use providers::provider_installation::SelectedProviderAdapter;
pub use verified_optimization_input::entry_settlement::{
    NativeProgramEntrySettlement, SelectedProgramEntryCall,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use verified_optimization_input::settlement::boundary_application_settlement::BoundaryApplicationSettlement;
pub use verified_optimization_input::NativeExecutionScope;
