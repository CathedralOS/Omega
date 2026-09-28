//! Prepare checked source, execute its build continuation, and seal selected execution.

pub(crate) mod build_continuation;
pub(crate) mod compile_thread;
pub(crate) mod execution_settlement;

pub use compilation_products::CheckedCompilation;

use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
use std::path::Path;

/// One `Independent` provider selection discovered while evaluating a checked
/// child's build, paired with the dependency package it names.
///
/// The named component is the selected provider type's owning package: the
/// dependency whose own checked compilation must publish a verified component
/// description for the selection to settle. `None` means the selected
/// provider carries no package identity — source-free or toolchain-owned — so
/// no dependency description can answer it; a selection naming the compiling
/// package itself is self-deployment, likewise unanswerable. Neither kind
/// falls back to a fused edge: the component-closure fence in provider
/// settlement still rejects them.
#[derive(Debug, Clone)]
pub struct IndependentComponentSelection {
    /// The exact evaluated selection the build authored.
    pub selection: provider_planning::ProviderSelection,
    /// The owning package of the selected provider type.
    pub component: Option<semantic_vocabulary::PackageKeyIdentity>,
}

/// Post-evaluation discovery one checked compile reports beside its verdict:
/// the `Independent` provider selections the evaluated build authored and
/// the usage of the evaluation that produced them. Both are recorded between
/// build evaluation and provider settlement's component-closure fence, so
/// they survive a rejection the fence issues. A settled compile reports the
/// same evaluation through its retained review instead.
#[derive(Debug, Default)]
pub struct IndependentComponentDiscovery {
    /// Every `Independent` selection, paired with the dependency package it
    /// names. Empty when the build selected none.
    pub selections: Vec<IndependentComponentSelection>,
    /// The sponsored build evaluation this attempt consumed. A caller that
    /// publishes the named components and compiles again reconciles it as
    /// discarded session consumption against the shared sponsor.
    pub evaluation_usage: Option<build_evaluation::BuildEvaluationUsage>,
}

pub use build_evaluation::RestrictedBuildGrants;

/// Inputs for checked-Psi compilation, without native publication authority.
/// All requests use the same source identity, package admission, and
/// sponsored build execution checks.
pub struct CheckedCompileRequest {
    /// Physical source entrypoint.
    pub root_path: std::path::PathBuf,
    /// Explicit target selection; `None` preserves targetless semantic checking.
    pub target_name: Option<String>,
    /// The admitted build execution profile: the profile build-scope sources
    /// (the build entry and its root-local helpers) are checked for, distinct
    /// from the product target. `None` admits the compiler host, which is
    /// where this request's build machines execute; a host no catalogued
    /// profile describes is admitted unprofiled rather than panicking or
    /// guessing a foreign profile.
    pub build_execution_profile: Option<target::TargetProfile>,
    /// Complete reconciled package graph; dependency declarations are not acquired.
    pub package_inputs: Option<PackageCompilationInputs>,
    /// Writable build staging root, separate from immutable package snapshots.
    pub build_dir: Option<std::path::PathBuf>,
    /// Caller-owned staging account shared across a review session.
    pub filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    /// Deterministic evaluator work account, independent of filesystem custody.
    pub evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    /// When present, the build occurrence executes against a captured
    /// immutable source snapshot and must complete each required output as a
    /// sealed regular file before its result may publish.
    pub build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    /// Release rollback subtracted from the authored optimization selection at
    /// each phase boundary this child executes. The authored selection remains
    /// the retained identity; the request can only remove exact rules it names.
    pub optimization_rollback: crate::checked::OptimizationRollback,
    /// Optional immutable source frontier from an earlier successful child.
    /// It supplies source preparation, never this activation's authority.
    pub prepared_source: Option<PreparedCheckedSource>,
    /// Return the source frontier after successful checking for another child.
    pub retain_prepared_source: bool,
    /// Occurrence-bound consent this activation's projected restricted build
    /// requests join before their own build effects execute. `None` admits
    /// without consulting retained acceptance and issues no grants.
    pub restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    /// Collect the internal stage ladder into the produced
    /// `CheckedCompilation`'s timing record. Off by default: the ladder
    /// measures nothing until a caller asks for it, and the rows stay on the
    /// checked record until a report-facing leg prints them. When the
    /// request reuses a caller-prepared source, collection follows how that
    /// source was prepared; this flag steers only the request-owned fresh
    /// preparation.
    pub collect_timings: bool,
    /// The consumer-review discovery pass admits a selected entry whose
    /// `Service` fields have no Fused provider selection yet — nominating
    /// those selections is what the pass exists for. Unsettled fields stay
    /// unestablished instead of rejecting; the bound recompile with its
    /// nominated bindings still derives or diagnoses every field.
    pub permit_unsettled_fused_service_fields: bool,
}

/// One checked attempt, including observations that survive a rejected verdict.
/// Source reuse publishes only on success; evaluated component selections and
/// sponsored usage survive a later provider-settlement rejection. No output is
/// written through the request or lost when the worker returns a diagnostic.
#[must_use]
pub struct CheckedCompileOutcome {
    pub result: Result<CheckedCompilation, Vec<Diagnostic>>,
    pub prepared_source: Option<PreparedCheckedSource>,
    pub independent_components: IndependentComponentDiscovery,
}

impl CheckedCompileRequest {
    /// Select source and optional target without package or sponsor inputs.
    pub fn new(root_path: &Path, target_name: Option<&str>) -> Self {
        Self {
            root_path: root_path.to_owned(),
            target_name: target_name.map(str::to_owned),
            build_execution_profile: None,
            package_inputs: None,
            build_dir: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            build_snapshot: None,
            optimization_rollback: crate::checked::OptimizationRollback::default(),
            prepared_source: None,
            retain_prepared_source: false,
            restricted_build_grants: None,
            collect_timings: false,
            permit_unsettled_fused_service_fields: false,
        }
    }

    /// Run this owned request on the compiler stack and return its whole outcome.
    /// Early admission failures return no source frontier or build observations.
    pub fn compile(self) -> CheckedCompileOutcome {
        CHECKED_COMPILATION_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        compile_thread::run_on_compile_thread(move || compile_checked_worker(self)).unwrap_or_else(
            |diagnostics| CheckedCompileOutcome {
                result: Err(diagnostics),
                prepared_source: None,
                independent_components: IndependentComponentDiscovery::default(),
            },
        )
    }
}

/// Target-independent source and parse work retained for one or more exact
/// checked children.
///
/// The checkpoint owns no child authority. Each child must independently
/// supply package inputs, build staging, and exact target selection; source
/// assembly rejects a child whose immutable package source projection differs
/// from the one prepared here.
/// Clones share parsed storage; consuming the sole checkpoint moves that storage
/// into its child instead of copying the syntax trees.
#[derive(Clone)]
pub struct PreparedCheckedSource {
    root_path: std::path::PathBuf,
    source_checkpoint: ImmutableSourceParseCheckpoint,
    shared_timings: CompileTimings,
    /// Typed frontiers this checkpoint's children produced, keyed by the exact
    /// frontier inputs each child supplied. Clones share the map, so a child
    /// that arrives at frontier inputs an earlier child already lowered reuses
    /// that typed program instead of re-assembling and re-lowering identical
    /// source; per-activation custody admission and every post-frontier gate
    /// still run for the reusing child.
    frontier_memo: std::sync::Arc<TypedFrontierMemo>,
}

type TypedFrontierMemo = std::sync::Mutex<
    Vec<(
        package_compilation::SourceAssemblyInputs,
        build_continuation::TypedFrontier,
    )>,
>;

pub(crate) struct CheckedChildExecution<'a> {
    selected_target_profile: Option<target::TargetProfile>,
    /// `None` admits the compiler host without a catalogued profile.
    build_execution_profile: Option<target::TargetProfile>,
    package_inputs: Option<&'a PackageCompilationInputs>,
    build_dir: Option<&'a Path>,
    filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    optimization_rollback: crate::checked::OptimizationRollback,
    /// Consent binding this child's projected restricted build requests join
    /// before their own build effects execute.
    restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    /// This child runs a discovery pass: an entry's unsettled `Service` fields
    /// stay unestablished rather than rejecting the compile they were
    /// discovered to nominate.
    permit_unsettled_fused_service_fields: bool,
}

impl<'a> CheckedChildExecution<'a> {
    /// One target of an ordinary compile: the caller's package inputs, build
    /// staging and rollback, the compiler host as build execution profile, no
    /// sponsors or discovery. An explicit snapshot narrows build authority;
    /// otherwise a packaged root binding carrying the compiler-captured
    /// canonical Source metadata index is sealed package custody, and every
    /// build activation over it runs against a fresh private materialization
    /// of the captured inventory with no invocation roster -- exactly what the
    /// package manager's review route requests -- so a retained Terminal
    /// production and the review evidence it must rejoin record the same build
    /// observation identity. A binding without that index keeps its live root.
    pub(crate) fn for_target(
        selected_target_profile: Option<target::TargetProfile>,
        package_inputs: Option<&'a PackageCompilationInputs>,
        build_dir: &'a Path,
        optimization_rollback: &crate::checked::OptimizationRollback,
        build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    ) -> Self {
        let automatic_snapshot = package_inputs
            .filter(|inputs| inputs.canonical_source_metadata(inputs.root()).is_some())
            .map(|_| build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()));
        Self {
            selected_target_profile,
            build_execution_profile: target::TargetProfile::host_if_supported(),
            package_inputs,
            build_dir: Some(build_dir),
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            // The request narrows authority explicitly. Never replace it with
            // automatic package membership or silently fall back after failure.
            build_snapshot: build_snapshot.cloned().or(automatic_snapshot),
            optimization_rollback: optimization_rollback.clone(),
            restricted_build_grants: None,
            permit_unsettled_fused_service_fields: false,
        }
    }

    pub(crate) fn package_inputs(&self) -> Option<&'a PackageCompilationInputs> {
        self.package_inputs
    }

    /// Ordinary target compilation has no external sponsor or consent guard.
    /// Compare its complete build inputs through their package owner, including
    /// the effective automatic/explicit capture and the live output directory.
    pub(crate) fn build_execution_inputs(&self) -> package_compilation::BuildExecutionInputs {
        package_compilation::BuildExecutionInputs::for_unsponsored(
            self.package_inputs,
            self.build_execution_profile,
            self.build_snapshot.as_ref(),
            self.build_dir,
        )
    }

    pub(crate) fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    pub(crate) fn optimization_rollback(&self) -> &crate::checked::OptimizationRollback {
        &self.optimization_rollback
    }

    pub(crate) fn permit_unsettled_fused_service_fields(&self) -> bool {
        self.permit_unsettled_fused_service_fields
    }
}

impl PreparedCheckedSource {
    pub fn prepare(
        root_path: &Path,
        package_sources: Option<
            std::sync::Arc<package_compilation::PackageCompilationSourceInputs>,
        >,
        collect_timings: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::prepare_with_timing_collection(root_path, package_sources, collect_timings)
    }

    /// Prepare the immutable source frontier, collecting the internal stage
    /// ladder into the accumulator the produced checked compilations seal
    /// when `collect_timings` is set. The rows stay on the checked record
    /// until a report-facing leg prints them.
    pub fn prepare_with_timing_collection(
        root_path: &Path,
        package_sources: Option<
            std::sync::Arc<package_compilation::PackageCompilationSourceInputs>,
        >,
        collect_timings: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Source discovery accepts no target attachments. Adapt the shared graph
        // to the frontend's package-routing view without consulting a child.
        let package_inputs = package_sources
            .map(|sources| PackageCompilationInputs::from_parts(sources, Default::default()))
            .transpose()
            .map_err(|errors| {
                errors
                    .into_iter()
                    .map(|error| Diagnostic::error(error.to_string()))
                    .collect::<Vec<_>>()
            })?;
        let mut shared_timings = if collect_timings {
            CompileTimings::enabled()
        } else {
            CompileTimings::default()
        };
        let source_checkpoint = ImmutableSourceParseCheckpoint::prepare(
            root_path,
            package_inputs.as_ref(),
            &mut shared_timings,
        )?;
        Ok(Self {
            root_path: root_path.to_owned(),
            source_checkpoint,
            shared_timings,
            frontier_memo: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        })
    }

    /// Prepare one admitted child's complete typed input before Build execution.
    /// Reuse never skips the child's live package-custody validation.
    pub(crate) fn prepare_frontier(
        self,
        child: &CheckedChildExecution<'_>,
    ) -> Result<(build_continuation::TypedFrontier, CompileTimings), Vec<Diagnostic>> {
        let key = package_compilation::SourceAssemblyInputs::new(
            child.package_inputs,
            child.build_execution_profile,
        );
        // The sole remaining checkpoint has no future consumer to memoize for.
        let retain_frontier = std::sync::Arc::strong_count(&self.frontier_memo) > 1;
        let frontier_memo = self.frontier_memo.clone();
        let retained = frontier_memo
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .find(|(stored_key, _)| *stored_key == key)
            .map(|(_, frontier)| frontier.clone());
        if let Some(frontier) = retained {
            if let Some(inputs) = child.package_inputs {
                inputs.validate_for_compilation(
                    &self.root_path,
                    &package_compilation::source_imports::bundled_core_root()?,
                )?;
            }
            TYPED_FRONTIER_REUSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok((frontier, self.shared_timings));
        }
        SOURCE_ASSEMBLY_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut timings = self.shared_timings;
        let assembly_started = std::time::Instant::now();
        let (source_file_count, syntax) = self
            .source_checkpoint
            .assemble(child.package_inputs, &mut timings)?;
        timings.add_completed(
            artifacts::compile_timings::SOURCE_ASSEMBLY,
            assembly_started.elapsed().as_micros(),
            artifacts::allocations::AllocationDelta::default(),
        );
        let frontend_started = std::time::Instant::now();
        let frontier = build_continuation::lower_typed_frontier(
            source_file_count,
            syntax,
            child
                .selected_target_profile
                .map(target::TargetProfile::target_name),
            child
                .build_execution_profile
                .map(target::TargetProfile::target_name),
            child.package_inputs,
            &mut timings,
        )?;
        // Preserve the report's inclusive Build ladder after separating preparation.
        timings.add_completed(
            artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
            frontend_started.elapsed().as_micros(),
            artifacts::allocations::AllocationDelta::default(),
        );
        if retain_frontier {
            let mut entries = frontier_memo
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            entries.retain(|(stored_key, _)| *stored_key != key);
            entries.push((key, frontier.clone()));
        }
        Ok((frontier, timings))
    }

    /// Admit one child's package inputs against this source frontier. Parsing
    /// is shared, but generated build results belong to their execution
    /// profile and each dependency-generated bundle to one target, so both are
    /// validated per child before any assembly uses them.
    pub(crate) fn admit_child(
        &self,
        child: &CheckedChildExecution<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        // Validate on every child, including prepared-source reuse: parsing is
        // shared, but generated build results belong to their execution profile.
        // This must precede assembly of the dependency's generated declarations.
        if let Some(inputs) = child.package_inputs {
            if inputs.compilation_purpose() == build_declarations::DependencyPurpose::Build
                && (child.build_execution_profile.is_none()
                    || child.selected_target_profile != child.build_execution_profile)
            {
                return Err(vec![Diagnostic::error(
                    "build-purpose package compilation requires its target to equal the admitted build execution profile",
                )]);
            }
            inputs
                .validate_dependency_generated_source_execution_profile(
                    child.build_execution_profile,
                )
                .map_err(|errors| {
                    errors
                        .into_iter()
                        .map(|error| Diagnostic::error(error.to_string()))
                        .collect::<Vec<_>>()
                })?;
        }
        // Each dependency-generated bundle was produced for one target; a
        // product bundle must belong to this child's target and a build bundle
        // to its build execution profile.
        if let Some(inputs) = child.package_inputs {
            inputs
                .validate_dependency_generated_source_target(child.selected_target_profile)
                .map_err(|errors| {
                    errors
                        .into_iter()
                        .map(|error| Diagnostic::error(error.to_string()))
                        .collect::<Vec<_>>()
                })?;
        }
        Ok(())
    }
}

/// Compile source through checked Psi on the compiler worker stack.
/// Returns checked semantics and selected build evidence, without backend lowering
/// or native output. Build execution may stage generated sources in its admitted root.
/// Call [`CheckedCompileRequest::compile`] when retaining preparation or discovery
/// from a rejected attempt; this convenience projection returns only the verdict.
pub fn compile_to_checked(
    request: CheckedCompileRequest,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    request.compile().result
}

static CHECKED_COMPILATION_CALLS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Monotonic count of checked-compilation requests entered, so a test can
/// witness that a review pass compiled each package occurrence once.
#[doc(hidden)]
pub fn checked_compilation_call_count() -> u64 {
    CHECKED_COMPILATION_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

static TYPED_FRONTIER_REUSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

static SOURCE_ASSEMBLY_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Monotonic count of source-set assemblies entered through a prepared checked
/// source. A memoized typed frontier serves its child without another
/// assembly, so the delta across one multi-target request counts how many
/// times the unioned source set was actually assembled.
#[doc(hidden)]
pub fn source_assembly_invocation_count() -> u64 {
    SOURCE_ASSEMBLY_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Monotonic count of typed frontiers a checked child reused from an earlier
/// child of its prepared source checkpoint, so a test can witness that a
/// review pass whose bindings joined after the frontier sealed did not
/// re-assemble and re-lower identical source.
#[doc(hidden)]
pub fn typed_frontier_reuse_count() -> u64 {
    TYPED_FRONTIER_REUSES.load(std::sync::atomic::Ordering::Relaxed)
}

static SETTLED_CHECK_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SETTLED_CHECK_MICROS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub use build_evaluation::preliminary_check_totals;

/// Monotonic `(calls, wall microseconds)` totals of the settled
/// `lower_typed_trees` checks, so a review pass report can show what the
/// settled checkpoint costs beside the preliminary gate.
#[doc(hidden)]
pub fn settled_check_totals() -> (u64, u64) {
    (
        SETTLED_CHECK_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        SETTLED_CHECK_MICROS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Monotonic count of pre-build declaration-authority checks that actually
/// executed, excluding requests `lower_typed_trees` served from its
/// exact-input check memo. A review pass's `preliminary_check_totals` call
/// delta counts every request; this delta counts only the misses, so their
/// difference names the memo-served repeats of an identical frozen program.
#[doc(hidden)]
pub fn executed_preliminary_check_count() -> u64 {
    typed_trees_to_checked_trees::RepeatedCheckRetention::executed_preliminary_check_count()
}

/// Monotonic count of settled whole-program checks that actually executed,
/// excluding requests the exact-input check memo served. A second review pass
/// over an unchanged closure still issues its settled requests, but a delta of
/// zero here witnesses that none of them ran the check again.
#[doc(hidden)]
pub fn executed_settled_check_count() -> u64 {
    typed_trees_to_checked_trees::RepeatedCheckRetention::executed_settled_check_count()
}

/// Account one settled whole-program check's wall time.
pub(crate) fn record_settled_check(elapsed: std::time::Duration) {
    SETTLED_CHECK_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    SETTLED_CHECK_MICROS.fetch_add(
        u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// The checked request's feed-forward route. Before Build completes there is
/// no discovery to return. After it completes, settlement and source-sealing
/// failures remain inside the outcome beside the consumed Build observations.
fn compile_checked_worker(
    request: CheckedCompileRequest,
) -> Result<CheckedCompileOutcome, Vec<Diagnostic>> {
    // Validate target selection before loading source, including fresh requests.
    let selected_target_profile = request
        .target_name
        .as_deref()
        .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let prepared = match request.prepared_source {
        Some(prepared) => prepared,
        None => PreparedCheckedSource::prepare_with_timing_collection(
            &request.root_path,
            request
                .package_inputs
                .as_ref()
                .map(PackageCompilationInputs::source_inputs),
            request.collect_timings,
        )?,
    };
    if request.root_path != prepared.root_path {
        return Err(vec![Diagnostic::error(
            "checked child compilation root does not match its prepared source checkpoint",
        )]);
    }
    let package_inputs = request.package_inputs.as_ref();
    let child = CheckedChildExecution {
        selected_target_profile,
        build_execution_profile: request
            .build_execution_profile
            .or_else(target::TargetProfile::host_if_supported),
        package_inputs,
        build_dir: request.build_dir.as_deref(),
        filesystem_sponsor: request.filesystem_sponsor,
        evaluation_sponsor: request.evaluation_sponsor,
        build_snapshot: request.build_snapshot,
        optimization_rollback: request.optimization_rollback.clone(),
        restricted_build_grants: request.restricted_build_grants,
        permit_unsettled_fused_service_fields: request.permit_unsettled_fused_service_fields,
    };
    let retained_source = request.retain_prepared_source.then(|| prepared.clone());
    prepared.admit_child(&child)?;
    let (frontier, mut timings) = prepared.prepare_frontier(&child)?;
    let build_started = std::time::Instant::now();
    let (built, sources) = build_continuation::evaluate_build_and_continue(
        &request.root_path,
        child,
        frontier,
        &mut timings,
    )?;
    timings.add_completed(
        artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
        build_started.elapsed().as_micros(),
        artifacts::allocations::AllocationDelta::default(),
    );
    let independent_components = IndependentComponentDiscovery {
        evaluation_usage: built.computed_build_config.evaluation_usage,
        selections: built
            .computed_build_config
            .config
            .provider_selections
            .iter()
            .filter(|selection| {
                selection.composition_mode == provider_planning::CompositionMode::Independent
            })
            .map(|selection| IndependentComponentSelection {
                selection: selection.clone(),
                component: selection.provider_type.package,
            })
            .collect(),
    };
    let settlement_started = std::time::Instant::now();
    let result = execution_settlement::check_selected_execution(
        built,
        selected_target_profile,
        package_inputs,
        &request.optimization_rollback,
        &mut timings,
        request.permit_unsettled_fused_service_fields,
    );
    timings.add_completed(
        artifacts::compile_timings::EXECUTION_SETTLEMENT,
        settlement_started.elapsed().as_micros(),
        artifacts::allocations::AllocationDelta::default(),
    );
    let result = result.and_then(|execution| {
        CheckedCompilation::seal(execution, sources, package_inputs, timings)
    });
    Ok(CheckedCompileOutcome {
        prepared_source: if result.is_ok() {
            retained_source
        } else {
            None
        },
        result,
        independent_components,
    })
}
