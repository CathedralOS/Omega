//! Build admission, execution, and checked result assembly.
//!
//! [`admit_build_program`] fixes the exact entry, target inputs, and authority
//! before issuing an opaque [`AdmittedBuildProgram`]. Its consuming execution
//! runs that checkpoint once, checks output custody, and assembles one
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
//! Start at `admitted_build_program.rs`: the admission request, the
//! admitted program and `admit_build_program`. `execution` runs an admitted
//! program and assembles its checked result. `admission/` owns what the build
//! program declares and selects (selection, target machines, vocabulary,
//! declarations, wire protocol, behavior exclusions, configuration), and
//! `evidence/` owns the evaluation evidence execution consults (observations
//! and their identity, filesystem scope and output custody).
//! `optimization` and `provider_settlement` own their own admissions.

mod admission;
mod admitted_build_program;
mod evidence;
mod execution;
mod optimization;
mod provider_settlement;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(test)]
mod tests;

pub use admission::behavior_exclusions::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, BehaviorExclusion,
    BehaviorExclusionReport, BehaviorExclusionVerdict, BehaviorExclusions, BoundaryServiceOwners,
    EvidenceGap, EvidenceGapKind, ProhibitedBehavior, ProhibitedSite,
    authored_behavior_exclusion_set, authored_behavior_exclusion_set_in,
    establish_behavior_exclusions, establish_behavior_exclusions_with_owners,
};
pub use admission::configuration::{
    ApplicationIdentifier, BuildConfig, HostedApplicationIntent, PccRequests,
};
pub use admission::declarations::{
    WireCompatibilityDemand, harvest_behavior_exclusions, harvest_provider_selections,
    harvest_root_grants,
};
pub use admission::selection::root_bindings::RootBinding;
pub use admission::selection::{
    SelectedCompilerProgramEntry, SelectedProgramEntry, SelectedProgramEntryCallingPlans,
    program_entry_semantic_binding_role, select_compiler_program_entry,
    selected_program_entry_machine, validate_selected_program_entry_calling_plan,
    validate_selected_program_entry_shape,
};
pub use admission::target_machines;
pub use admission::vocabulary::is_build_machine;
pub use admission::wire_protocol::validate_wire_protocol;
pub use admitted_build_program::{
    AdmittedBuildAuthorityVerdict, AdmittedBuildProgram, AdmittedBuildProgramDisposition,
    AdmittedBuildTargetInputs, BuildSnapshotCapture, BuildSnapshotRequest, ComputedBuildConfig,
    RestrictedBuildBounds, RestrictedBuildGrant, RestrictedBuildGrantRoot,
    RestrictedBuildOperation, RestrictedBuildRequest, admit_build_program,
};
pub use build_output::BuildStagedOutputEntryKind;
pub use evidence::filesystem_scope::preparation::prepare_filesystem_scope;
pub use evidence::filesystem_scope::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};
pub use evidence::observation_identity::BuildObservationIdentity;
pub use evidence::observations::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildActivation, BuildCanonicalSourceMetadataIdentity,
    BuildCapturedSourceInventory, BuildEvaluationUsage, BuildFilesystemAuthorizedPath,
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusal, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemRoot, BuildIncludedSourceHandoff, BuildObservationSummary,
    BuildRequiredOutputSettlement,
};
pub use execution::execute_admitted_build_program;
pub use provider_settlement::{
    CheckedProviderSelection, settle_checked_providers, verify_independent_component_descriptions,
};
