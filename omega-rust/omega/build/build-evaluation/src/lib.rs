//! Build admission, execution, and checked result assembly.
//!
//! [`admit_build_program`] fixes the exact entry, target inputs, and authority
//! before issuing an opaque [`AdmittedBuildProgram`]. Its consuming execution
//! runs that checkpoint, checks replay and output custody, and assembles one
//! [`ComputedBuildConfig`]. Selection and evidence operations live with their
//! domain owners; this root retains the admission-to-result route.
//!
//! Build configuration (wiki/spec/build/configuration.md):
//! image facts come from `build.omg`'s augmenting machine, never from an
//! invented config grammar. When the program (build.omg is ordinary source,
//! auto-included next to main.omg) defines the conventionally-named free
//! machine `build(build: &mut Build)`, the compiler evaluates it at build time
//! (purity-gated, the L0 engine) with a ZII `Build` and reads the augmented
//! value back:
//!
//! ```omega
//! data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
//! data Optimization { case ControlFlowCleanup; /* ... */ }
//! data Optimizations { control_flow_cleanup: u8 in Trapping; /* ... */ }
//! data Build { subsystem: Subsystem; freestanding: bool; optimizations: Optimizations; }
//! machine build(build: &mut Build) {
//!     build.subsystem = Subsystem::EfiApplication;
//!     build.freestanding = true;
//!     build.optimizations.enable(Optimization::ControlFlowCleanup);
//! }
//! ```
//!
//! - `subsystem` is loader METADATA (a PE header u16 the compiler copies; it
//!   does not select the emitter). The ZII zero case is `Console` -- the
//!   correct default falls out of the type. `Unspecified(value)` is the
//!   escape hatch: any loader value a platform invents, with no compiler
//!   release.
//! - `freestanding` ("trust no host packages") is stated as itself --
//!   previously fused into the `efi_application` name.
//! - Absent build.omg == an empty `build` machine == the zero `Build`: the
//!   hosted console default.
//! - `optimizations` is an exact set of individually named transformations.
//!   It is empty by default; duplicates reject rather than acting like levels.
//! - `builder.roots.bind(target::ProgramEntry, Exact::machine);` executes
//!   through the original Build activation, including local helpers/reborrows.
//!   Executed requests rejoin their lexical owner before exact product selection.
//!
//! `admitted_build_program.rs` is the root: the admission request, the
//! admitted program and `admit_build_program`. `execution.rs` runs an admitted
//! program and assembles its checked result. The other modules each own one
//! input or evidence family the two consult: configuration, declarations,
//! filesystem scope, observations, optimization, provider settlement, replay
//! eligibility and records, selection, target machines, vocabulary and the
//! wire protocol.

mod admitted_build_program;
mod behavior_exclusions;
mod configuration;
mod declarations;
mod execution;
mod filesystem_scope;
mod observation_identity;
mod observations;
mod optimization;
mod provider_settlement;
mod replay_eligibility;
mod replay_record;
mod selection;
pub mod target_machines;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(test)]
mod tests;
mod vocabulary;
mod wire_protocol;

pub use admitted_build_program::{
    AdmittedBuildAuthorityVerdict, AdmittedBuildProgram, AdmittedBuildProgramDisposition,
    AdmittedBuildTargetInputs, BuildSnapshotRequest, ComputedBuildConfig, admit_build_program,
    reject_uncompiled_generated_sources,
};
pub use behavior_exclusions::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, BehaviorExclusion,
    BehaviorExclusionReport, BehaviorExclusionVerdict, BehaviorExclusions, EvidenceGap,
    EvidenceGapKind, ProhibitedBehavior, ProhibitedSite, authored_behavior_exclusion_set,
    establish_behavior_exclusions,
};
pub use configuration::{ApplicationIdentifier, BuildConfig, HostedApplicationIntent, PccRequests};
pub use declarations::{
    WireCompatibilityDemand, harvest_behavior_exclusions, harvest_provider_selections,
    harvest_root_grants,
};
pub use execution::execute_admitted_build_program;
pub use filesystem_scope::preparation::prepare_filesystem_scope;
pub use filesystem_scope::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};
pub use observation_identity::BuildObservationIdentity;
pub use observations::{
    BUILD_FILESYSTEM_REPLAY_VERDICT_SCHEMA_VERSION, BUILD_OBSERVATION_SCHEMA_VERSION,
    BuildCanonicalSourceMetadataIdentity, BuildCapturedSourceInventory, BuildEvaluationUsage,
    BuildFilesystemAuthorizedPath, BuildFilesystemByteOperand, BuildFilesystemGrantAccess,
    BuildFilesystemGrantRefusal, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemMetadataObservation, BuildFilesystemMetadataObservationKind,
    BuildFilesystemMutableByteOperand, BuildFilesystemMutableByteOperandResolution,
    BuildFilesystemMutableI64Operand, BuildFilesystemMutableI64OperandResolution,
    BuildFilesystemObservedByteRegion, BuildFilesystemObservedByteRegionKind,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationObservationClass,
    BuildFilesystemOperationResult, BuildFilesystemPathLikeOperand, BuildFilesystemProvider,
    BuildFilesystemReplayDisposition, BuildFilesystemReplayVerdict, BuildFilesystemReturnedPath,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildFilesystemScalarOperandValue, BuildIncludedSourceHandoff, BuildObservationClass,
    BuildObservationSummary, BuildReplayActivation, BuildRequiredOutputSettlement,
};
pub use provider_settlement::{CheckedProviderSelection, settle_checked_providers};
pub use replay_record::{
    BuildFilesystemReplayRecordError, BuildFilesystemReplayRecordLimits,
    ReviewOnlyBuildFilesystemReplayRecord, capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};
pub use selection::root_bindings::RootBinding;
pub use selection::{
    SelectedCompilerProgramEntry, SelectedProgramEntry, SelectedProgramEntryCallingPlans,
    program_entry_semantic_binding_role, select_compiler_program_entry,
    selected_program_entry_machine, validate_selected_program_entry_calling_plan,
    validate_selected_program_entry_shape,
};
pub use vocabulary::is_build_machine;
pub use wire_protocol::validate_wire_protocol;
