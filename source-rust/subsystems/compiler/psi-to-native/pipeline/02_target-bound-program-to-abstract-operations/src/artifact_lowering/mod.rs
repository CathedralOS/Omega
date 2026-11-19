#![forbid(unsafe_code)]

//! Lower the selected, authorized Psi bodies and admit their providers and entry.

mod artifact_admission;
mod entry_receiver;
pub use entry_receiver::admit_executable_entry;
mod lowering;
mod native_execution_scope;
pub(crate) use native_execution_scope::native_admission;
mod retained_terminal;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub use native_admission::{
    AdmittedNativeRealization, NativeAdmissionRequest,
    admit_prepared_native_realization,
};
pub use provider_admission::NativeCallbackArgumentProposal;
pub use retained_terminal::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    PreparedNativeExecution, RetainedNativeProviderInputs, RetainedTerminalArtifact,
    TerminalNativeProposalRequest, TerminalNativeRealizationProposal, lower_native_callable,
    lower_native_execution, prepare_native_providers, project_terminal_native_realization_proposal,
};

pub(crate) use artifact_admission::ArtifactLoweringError;
pub use artifact_admission::{
    NativeExecutionSelection, NativeScalarApplications, TerminalAccessViewEstablishment,
    VerifiedNativeArtifactInput,
};
pub use provider_admission::SelectedProviderAdapter;
pub use verified_optimization_input::NativeExecutionScope;
pub use verified_optimization_input::entry_settlement::{
    NativeProgramEntrySettlement, SelectedProgramEntryCall, ValidatedNativeProgramEntrySettlement,
    validate_native_program_entry_settlement,
};
pub use verified_optimization_input::settlement::boundary_application_settlement::BoundaryApplicationSettlement;
