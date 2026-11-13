#![forbid(unsafe_code)]

/*
Source-free native compiler: lower retained Terminal and realization inputs to
an executable image or admitted Terminal-runtime provisions.

`compile` and `admit_runtime` prepare the supplied realization, admit entry and
providers through Omega 00, and drive the abstract, target, and physical stages
sequenced in `pipeline.rs`. Callback bodies and independent component exports
traverse the same provider admission and physical pipeline using their own
artifact-local native proposal and no application entry.
*/

/*
Source compilation, Build execution, and package review remain outside this
dependency boundary: `native-compiler` never reads typed or checked trees to
repair a missing Terminal fact. `application.rs` owns source-free Terminal
application preparation and package-permission admission, shared by artifact
publication and native realization.
*/

mod application;
mod compile;
mod components;
mod observations;
mod pipeline;
mod preparation;
mod realization;
mod request;
mod scalar_completion;

#[cfg(test)]
mod pipeline_boundary;

pub use application::{TerminalApplication, admit_package_permissions};
pub use compile::{admit_runtime, compile};
pub use components::ComponentCompilation;
pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use observations::{NativePipelineInvocation, NativePipelineUnit, native_pipeline_invocations};
pub use preparation::{PreparedNativeCompilation, prepare};
pub use realization::RealizedNativeProduct;
pub use request::NativeCompileRequest;
pub use scalar_completion::NativeScalarCompletion;
pub use terminal_psi_to_abstract_operations::provider_admission::{
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy,
};
pub use terminal_psi_to_abstract_operations::{
    BoundaryApplicationSettlement, ComponentExportInput, ComponentNativeInputs,
    NativeProgramEntrySettlement, TerminalNativeProposalRequest, decode_installation_assumptions,
    encode_installation_assumptions, retain_native_derivation_inputs,
};
