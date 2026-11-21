//! Lower selected, authorized Psi bodies to abstract operations and admit their providers and entry.
#![forbid(unsafe_code)]

mod admit_providers;
mod entry_receiver;
mod operations;
mod selected_execution;
mod selected_input;

pub use admit_providers::{
    AdmittedNativeRealization, NativeAdmissionRequest, RetainedNativeProviderInputs,
    admit_prepared_native_realization, lower_native_root,
};
pub use entry_receiver::admit_executable_entry;
pub use selected_execution::lower_native_callable;
pub use selected_input::{NativeScalarApplications, VerifiedNativeArtifactInput};

use provider_admission::NativeCallbackArgumentProposal;
use selected_input::{
    ArtifactLoweringError, NativeExecutionSelection, TerminalAccessViewEstablishment,
};
use target_bound_program::entry_settlement::{
    SelectedProgramEntryCall, validate_native_program_entry_settlement,
};
use target_bound_program::{NativeExecutionScope, SelectedProviderAdapter};

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
