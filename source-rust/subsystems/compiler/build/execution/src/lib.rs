//! Admit a checked Build invocation, run it in the interpreter with its granted
//! services, and return its settings, requests and execution receipts.

mod activation;
mod admission;
mod build_evaluation;
pub mod host;

pub use activation::preparation::PreparedBuildExecution;
pub use activation::{
    BuildDependencySelection, BuildSourceResult, ReturnedBuild, TerminalBuildActivation,
    TerminalBuildHost, TerminalBuildStep,
};
pub use admission::checking::settle_rejoined_build_slice;
use admission::program::{AdmittedBuildProgram, admit_build_program};
pub use build_evaluation::{
    BuildEvaluationRequest, BuildStageRequest, EvaluatedBuild, PreparedBuildEvaluation, SourceBuildDeclaration,
    RestrictedBuildGrants, prepare_build_evaluation,
};
pub use build_output::BuildStagedOutputEntryKind;
pub use behavior_exclusion_requests::AuthoredBehaviorExclusion;
pub use package_compilation::build_execution::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY,
    BuildActivation, BuildCanonicalSourceMetadataIdentity, BuildCapturedSourceInventory,
    BuildEvaluationUsage, BuildFilesystemAuthorizedPath, BuildFilesystemGrantAccess,
    BuildFilesystemGrantRefusal, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemRoot, BuildIncludedSourceHandoff, BuildMachineFilesystemScope,
    BuildNamedInputInventory, BuildObservationIdentity, BuildObservationSummary,
    BuildRequiredOutputSettlement, BuildSnapshotCapture, BuildSnapshotRequest,
    RestrictedBuildRequest, prepare_filesystem_scope,
};
pub use build_evaluation_result::{
    BuildConfig, BuildConstants, BuildSubjectValidation, CompileFailure, ComputedBuildConfig,
};
pub use source_compiler::{ProducedBuildProvider, WireCompatibilityDemand};
pub use target::{ApplicationIdentifier, HostedApplicationIntent};

mod program_entry_selection;
pub use source_compiler::RootBinding;
pub use program_entry_selection::{SelectedProgramEntry, selected_program_entry_machine};

#[cfg(test)]
#[path = "../../../../../../tests/support/frontend.rs"]
mod frontend_fixture;
