//! Lower selected, authorized Psi bodies to abstract operations and admit their providers and entry.
#![forbid(unsafe_code)]

mod admit_providers;
mod entry_receiver;
mod operations;
mod selected_execution;
mod selected_input;

pub use admit_providers::{
    AdmittedNativeRealization, NativeAdmissionRequest, admit_prepared_native_realization,
};
pub use entry_receiver::admit_executable_entry;
pub use selected_execution::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    PreparedNativeExecution, RetainedNativeProviderInputs, RetainedTerminalArtifact,
    TerminalNativeProposalRequest, TerminalNativeRealizationProposal, lower_native_callable,
    lower_native_execution, prepare_native_providers, project_terminal_native_realization_proposal,
};
pub use selected_input::{NativeScalarApplications, VerifiedNativeArtifactInput};
pub use verified_optimization_input::entry_settlement::{
    NativeProgramEntrySettlement, ValidatedNativeProgramEntrySettlement,
};
pub use verified_optimization_input::settlement::boundary_application_settlement::BoundaryApplicationSettlement;

use provider_admission::{NativeCallbackArgumentProposal, SelectedProviderAdapter};
use selected_input::{
    ArtifactLoweringError, NativeExecutionSelection, TerminalAccessViewEstablishment,
};
use verified_optimization_input::NativeExecutionScope;
use verified_optimization_input::entry_settlement::{
    SelectedProgramEntryCall, validate_native_program_entry_settlement,
};

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
