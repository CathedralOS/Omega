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
dependency boundary: `native-compiler` never reads typed trees to
repair a missing Terminal fact. `application.rs` owns source-free Terminal
application preparation and package-permission admission, shared by artifact
publication and native realization.
*/

pub use native_artifact::AdmittedRuntimeProgram;
pub use published_object_verification::NativeStageReplay;
mod program_entry;
pub use program_entry::PublishedNativeEntry;
mod published_application;
pub use backend_plan::{InstalledPackageProviders, TargetProviderInstallation};
pub use published_application::{
    PublishedApplicationInputs, SavedProgramTarget, prepare_published_application,
    prepare_saved_program,
};
mod application;
mod body_planning;
pub mod bridge_replay;
mod callable_planning;
mod compile;
mod components;
mod handler_order;
mod observations;
mod pipeline;
mod preparation;
mod realization;
mod request;
mod runtime_provisions;
mod scalar_completion;

#[cfg(test)]
mod physical_publication_tests;
pub use application::{TerminalApplication, admit_package_permissions};
pub use artifact_lowering::{
    BoundaryApplicationSettlement, ComponentExportInput, ComponentNativeInputs,
    NativeProgramEntrySettlement, TerminalNativeProposalRequest,
};
pub use compile::{admit_runtime, compile};
pub use components::ComponentCompilation;
pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use observations::{NativePipelineInvocation, NativePipelineUnit, native_pipeline_invocations};
pub use provider_admission::provider_admission::{
    CompilerIntrinsicPlanProvenance, NativeBindingInput, SourceEvaluatedImportSettlement,
    TerminalAuthorityPermissionPolicy, current_terminal_authority_policy,
};
pub use realization::RealizedNativeProduct;
pub use request::NativeCompileRequest;
pub use verified_optimization_input::{
    decode_installation_assumptions, encode_installation_assumptions,
    retain_native_derivation_inputs,
};

pub use provider_realization::{
    PublishedSelection, SelectedExecution, SelectedProviderRealization, SelectedProviderResult,
    select_published_providers,
};

// Published native entry selection and explicitly requested provider-input inspection.
pub use provider_realization::{
    plan_provider_execution_inputs, target_supply::TargetSupplyExecutionInputs,
};

pub use provider_realization::program_entry::{
    PublishedEntryNomination, SelectedCompilerProgramEntry,
    plan_published_entry_calling_conventions,
};
