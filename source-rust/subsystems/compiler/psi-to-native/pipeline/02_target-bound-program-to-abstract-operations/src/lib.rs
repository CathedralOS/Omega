//! Lower authorized Psi code to abstract operations for the selected execution.
#![forbid(unsafe_code)]

mod artifact_lowering;
pub use artifact_lowering::{
    AdmittedNativeComponentCall, BoundaryApplicationSettlement, ComponentExportInput,
    ComponentNativeInputs, NativeProgramEntrySettlement, PreparedNativeExecution,
    RetainedNativeProviderInputs, RetainedTerminalArtifact, TerminalNativeProposalRequest,
    TerminalNativeRealizationProposal, ValidatedNativeProgramEntrySettlement,
    project_terminal_native_realization_proposal,
};
pub use artifact_lowering::{
    AdmittedNativeRealization, NativeAdmissionRequest,
    NativeScalarApplications, VerifiedNativeArtifactInput, admit_executable_entry,
    admit_prepared_native_realization,
    lower_native_callable, lower_native_execution, prepare_native_providers,
};

#[cfg(any(test, feature = "test-support"))]
pub use artifact_lowering::test_support;
