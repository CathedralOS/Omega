//! Build admission, execution, and checked result assembly.
//!
//! [`prepare_build_evaluation`] selects the source-owned entry before semantic admission,
//! filesystem preparation and restricted-request consent. Its private admitted checkpoint
//! binds the exact program and entry before effects. The caller starts and steps
//! one retained Terminal activation, then rejoins its normal return with the
//! prepared evaluation. Only that completion produces [`EvaluatedBuild`], carrying
//! configuration, requested authority and source commitment together.
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
//! through the admitted Terminal interpreter with a ZII `Build` and reads the augmented
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
//! - `Build::bind(builder, target::ProgramEntry, Exact::machine);` executes
//!   through the original Build activation, including local helpers/reborrows.
//!   Executed requests rejoin their lexical owner before exact product selection.
//! - `Build::select_provider<Slot, Provider>(builder, mode)` likewise records only
//!   executed choices on the original Build value. Its static operands select
//!   product declarations without executing them; target-default declarations
//!   remain under their separate admitted target roster.
//!
//! Start at `build_evaluation.rs`: it admits the Build program, runs it and
//! reads the result back, in that order. The folders are those steps:
//!
//! - `admission/` checks the program the activation will consume before any
//!   effect; `program.rs` fixes its private checkpoint.
//! - `activation.rs` owns the activation the caller steps. `preparation`
//!   prepares its code and account, authenticates the returned invocation and
//!   assembles its checked result; `completed_pure_build` owns invocation-local
//!   completed Pure values, admitting each fresh before consumption and
//!   charging each replaced evaluation to the current account without
//!   fabricating interpreter execution or live-resource peaks.
//! - `host/` is what the running program may call: bindings and product
//!   queries, filesystem access and host requests.
//! - `settings.rs` decodes the Build settings after normal return.
//!
//! The activation retains every request through normal return;
//! completion requires exact source-request settlement. Bound compilations can
//! join retained acquisitions; anonymous discovery joins its returned requests
//! after source acquisition. An alias graph alone never discharges the ledger.
//! `compilation_products` owns the
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
//! including selection decoding and settlement belongs to the independent
//! `provider-planning` realization owner. This crate selects Build helpers for
//! the admitted execution target before ordinary Psi resolution.

mod activation;
mod admission;
mod build_evaluation;
mod calling_policy_execution;
pub mod host;
pub mod settings;

pub use activation::lender_preservation;
pub use activation::scalar_completion;

pub use activation::completed_pure_build::PureBuildCompilation;
pub use activation::preparation::PreparedBuildExecution;
pub use activation::{
    BuildDependencySelection, ReturnedBuild, TerminalBuildActivation, TerminalBuildHost,
    TerminalBuildStep,
};
use admission::program::{AdmittedBuildProgram, admit_build_program};
pub use admission::target_helpers::select_build_helpers;
pub use artifacts::compile_timings::counters::preliminary_check_totals;
pub use build_evaluation::{
    BuildEvaluationRequest, EvaluatedBuild, PreparedBuildEvaluation, RestrictedBuildGrants,
    prepare_build_evaluation,
};
pub use build_output::BuildStagedOutputEntryKind;
pub use build_time_evaluation::WireCompatibilityDemand;
pub use calling_policy_execution::evaluate_terminal_calling_policy;
pub use compilation_products::{BuildConfig, ComputedBuildConfig};
pub use effects::AuthoredBehaviorExclusion;
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
    PreparedCompilerProgramEntry, RootBinding, SelectedCompilerProgramEntry, SelectedProgramEntry,
    SelectedProgramEntryCallingPlans, prepare_compiler_program_entry,
    program_entry_semantic_binding_role, selected_program_entry_machine,
};
pub use target::{ApplicationIdentifier, HostedApplicationIntent};
pub use trust_model::AuthoredComponentAssumptionAcceptance;
