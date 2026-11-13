/*
Build admission, Terminal `build` machine execution, and checked result assembly
(`build-settings`).

We evaluate `build.omg` as ordinary Checked/Terminal Omega code rather than
inventing a separate configuration grammar (`wiki/spec/build/configuration.md`).
When a package or application defines the free entry `machine build(build: &mut Build)`,
we check and lower that program into a canonical Terminal Psi artifact, invoke
it inside the Terminal interpreter against a zero-initialized `Build` record,
service its host calls (`Build::bind`, `Build::select_provider`, filesystem
staging, and dependency source requests) on the original `Build` root, and read
the resulting configuration and authority requests back after normal return:

```omega
data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Optimization { case ControlFlowCleanup; /* ... */ }
data Optimizations { human_report: u8 in Saturating; selected: [u8 in Saturating; 55]; }
data Build { subsystem: Subsystem; freestanding: bool; optimizations: Optimizations; }

machine build(build: &mut Build) {
    build.subsystem = Subsystem::EfiApplication;
    build.freestanding = true;
    Optimizations::enable(&mut build.optimizations, Optimization::ControlFlowCleanup);
}
```

- `subsystem` is loader metadata (e.g. the PE header `u16` copied into the
  emitted image; it never selects a different backend emitter). Its zero case is
  `Console`, so an omitted `build.omg` or empty `build` machine yields the hosted
  console default. `Unspecified(value)` provides a platform escape hatch without
  requiring a compiler release.
- `freestanding` ("trust no host packages") is an explicit boolean field rather
  than being conflated with `EfiApplication`.
- `optimizations` is an exact set of individually named transformations (empty by
  default; duplicate selections reject rather than acting like coarse `-O`
  levels).
- `Build::bind(builder, target::ProgramEntry, Exact::machine)` and
  `Build::select_provider<Slot, Provider>(builder, mode)` execute through the
  original `Build` activation (including local helper functions and reborrows)
  and record choices only on the original root `Build` value.
*/

/*
Architecture & Ownership Boundaries:

Start at `build_evaluation.rs`: `prepare_build_evaluation` selects the
source-owned `build` entry before semantic admission, filesystem preparation, and
restricted-request consent. Its private admitted checkpoint (`AdmittedBuildProgram`)
binds the exact accepted invocation source and entry before any host effects occur. The
caller (`orchestration`) then starts and steps one retained `TerminalBuildActivation`
and rejoins its normal return (`ReturnedBuild`) with `PreparedBuildEvaluation::complete`
to produce `EvaluatedBuild`.

The crate is organized along that lifecycle:
- `admission/` owns source and package admission before effects. Build source
  preparation explicitly refuses until a structural producer supplies its real
  source, declaration-authority and constant/provider custody (`checking.rs`).
  `provider_execution.rs` admits actual selected runtime supplies, while
  `program.rs` binds a future accepted source to its private checkpoint.
- `activation.rs` and `activation/` own the step-by-step `TerminalBuildActivation`
  that `orchestration` drives (`preparation.rs`, `project.rs`,
  `provider_installation.rs`, `scalar_completion.rs`, `source_join.rs`,
  `source_result.rs`), plus invocation-local `Pure` build value reuse
  (`completed_pure_build.rs`), which charges each reused evaluation to the
  current work sponsor without fabricating interpreter execution or live-resource
  peaks.
- `host/` implements the host imports callable from `build(build: &mut Build)`:
  root bindings and product entry queries (`bindings.rs`, `native_bindings.rs`,
  `product_queries.rs`), scoped filesystem staging (`filesystem.rs`), and
  dependency source / restricted authority requests (`requests.rs`).
- `settings.rs` decodes the plain `Build` configuration fields from the
  interpreter's root structural parameter after normal return.
- `calling_policy_execution.rs` executes retained Terminal calling-policy
  machines on canonical ABI signatures without re-entering the source compiler.
- `via_binding_evaluation.rs` runs each ordinary source `via` producer once, for
  Build admission and for the compiler's Product preparation, and hands the
  result to provider planning to retain.
*/

mod activation;
mod admission;
mod build_evaluation;
mod calling_policy_execution;
pub mod host;
pub mod settings;
mod via_binding_evaluation;

pub use activation::scalar_completion;

pub use activation::completed_pure_build::PureBuildCompilation;
pub use activation::preparation::PreparedBuildExecution;
pub use activation::{
    BuildDependencySelection, BuildSourceResult, ReturnedBuild, TerminalBuildActivation,
    TerminalBuildHost, TerminalBuildStep,
};
pub use admission::checking::settle_rejoined_build_slice;
use admission::program::{AdmittedBuildProgram, admit_build_program};
pub use admission::target_helpers::select_build_helpers;
pub use artifacts::compile_timings::counters::preliminary_check_totals;
pub use build_evaluation::{
    BuildEvaluationRequest, EvaluatedBuild, PreparedBuildEvaluation, RestrictedBuildGrants,
    prepare_build_evaluation,
};
pub use build_output::BuildStagedOutputEntryKind;
pub use build_time_evaluation::WireCompatibilityDemand;
pub use calling_policy_execution::{CALLING_POLICY_EVALUATOR, evaluate_terminal_calling_policy};
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
pub use via_binding_evaluation::evaluate_checked_via_bindings;
