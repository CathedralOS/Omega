//! Build admission, execution, and checked result assembly.
//!
//! [`evaluate_build`] selects the source-owned entry before semantic admission,
//! filesystem preparation, restricted-request consent, and execution. Its private admitted checkpoint
//! binds the exact program and entry before effects. The returned [`EvaluatedBuild`]
//! carries configuration, requested authority and source commitment together.
//! Selection and evidence operations remain with their domain owners.
//! Every activation checks its helpers, independently of optional package inputs,
//! then consumes that checker-transformed program without respecialization.
//! With no entry, default settings and source identity require no activation;
//! final product checking remains responsible for the ordinary program.
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
//!     Optimizations::enable(&mut build.optimizations, Optimization::ControlFlowCleanup);
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
//! - `builder.select_provider<Slot, Provider>(mode)` likewise records only
//!   executed choices on the original Build value. Its static operands select
//!   product declarations without executing them; target-default declarations
//!   remain under their separate admitted target roster.
//!
//! Start at `build_evaluation.rs` for the complete activation protocol.
//! `completed_pure_build` owns invocation-local completed Pure values. Its
//! insertion boundary is private to this evaluator: fresh admission precedes
//! consumption, and each replaced evaluation receives a current-account logical
//! charge without fabricating interpreter execution or live-resource peaks.
//! `admitted_build_program.rs` fixes its private checkpoint; `execution` runs
//! that checkpoint and assembles its checked result. `compilation_products` owns the
//! retained Build records; admission and execution produce them without making
//! downstream product custody depend on this evaluator.
//! Trust grants, component assumptions and executed exclusions are checked by
//! realization trust; Psi owns wire-demand decoding and compatibility.
//! Package compilation's `build_execution` owner retains filesystem scope,
//! observation identities and output custody. Execution captures interpreter
//! observations and requests settlement; it cannot construct the summary fields.
//! The same owner projects restricted host requests from the admitted mode and
//! scope. These records describe requested authority; they never grant it.
//! Package compilation's `build_execution::build_value` owns Build vocabulary
//! admission, initial facet construction and evaluated settings, including
//! exact optimization selection. The compiler retains those settings directly
//! beside the independently checked selection receipts. Provider planning
//! including target declarations, selection decoding and settlement belongs to
//! the independent `provider-planning` realization owner.

mod admission;
mod admitted_build_program;
mod build_evaluation;
mod completed_pure_build;
mod execution;
#[cfg(test)]
mod tests;

use admitted_build_program::{AdmittedBuildProgram, admit_build_program};
pub use artifacts::compile_timings::counters::preliminary_check_totals;
pub use build_evaluation::{
    BuildEvaluationRequest, EvaluatedBuild, RestrictedBuildGrants, evaluate_build,
};
pub use build_output::BuildStagedOutputEntryKind;
pub use build_time_evaluation::WireCompatibilityDemand;
pub use compilation_products::{BuildConfig, ComputedBuildConfig};
pub use completed_pure_build::PureBuildCompilation;
pub use effects::AuthoredBehaviorExclusion;
use execution::execute_admitted_build_program;
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
pub use provider_planning::program_entry::{
    RootBinding, SelectedCompilerProgramEntry, SelectedProgramEntry,
    SelectedProgramEntryCallingPlans, program_entry_semantic_binding_role,
    select_compiler_program_entry, selected_program_entry_machine,
    validate_selected_program_entry_calling_plan, validate_selected_program_entry_shape,
};
pub use target::{ApplicationIdentifier, HostedApplicationIntent};
pub use trust_model::AuthoredComponentAssumptionAcceptance;
