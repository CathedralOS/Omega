//! Prepare checked source, execute its build continuation, and seal selected execution.

pub(crate) mod build_continuation;
pub(crate) mod checked_compilation;
pub(crate) mod compile_thread;
mod const_evaluation;
pub(crate) mod execution_settlement;
pub(crate) mod phase_transitions;

pub use checked_compilation::CheckedCompilation;

use crate::artifacts::compile_timings::CompileTimings;
use crate::compiler::sources::ImmutableSourceParseCheckpoint;
use crate::package_compilation::PackageCompilationInputs;
use diagnostics::Diagnostic;
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
    pub selection: crate::provider_planning::ProviderSelection,
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
    pub evaluation_usage: Option<crate::build_evaluation::BuildEvaluationUsage>,
}

/// Retained restricted-request consent a consuming caller binds to one
/// checked activation.
///
/// With a binding present, each restricted build-host request the admitted
/// build machine projects joins here — in issue order — before that
/// request's own build effect executes; a refusal fails the compile without
/// running it. The binding owns what it consults: the compile worker runs on
/// its own thread, so retained consent crosses as owned meaning, not a
/// borrow.
///
/// `None` is observation-only: requests still project into the compile's
/// retained custody, where a review presents or a lock retains them, but the
/// activation admits without consulting accepted request meaning. Audit-only
/// callers pass no binding and issue no grants.
pub trait RestrictedBuildGrants: Send {
    /// Join one projected restricted build request. `Err` carries the
    /// diagnostics the checked compile returns unchanged.
    fn admit(
        &mut self,
        request: &crate::build_evaluation::RestrictedBuildRequest,
    ) -> Result<(), Vec<Diagnostic>>;
}

/// Inputs for checked-Psi compilation, without native publication authority.
/// All requests use the same source identity, package admission, and
/// sponsored build execution checks.
pub struct CheckedCompileRequest<'a> {
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
    pub filesystem_sponsor: Option<crate::build_time_evaluation::BuildMachineFilesystemSponsor>,
    /// Deterministic evaluator work account, independent of filesystem custody.
    pub evaluation_sponsor: Option<crate::build_time_evaluation::BuildEvaluationSponsor>,
    /// When present, the build occurrence executes against a captured
    /// immutable source snapshot and must complete each required output as a
    /// sealed regular file before its result may publish.
    pub build_snapshot: Option<crate::build_evaluation::BuildSnapshotRequest>,
    /// Release rollback subtracted from the authored optimization selection at
    /// each phase boundary this child executes. The authored selection remains
    /// the retained identity; the request can only remove exact rules it names.
    pub optimization_rollback: crate::compiler::checked::OptimizationRollback,
    /// Optional destination for target-independent source preparation. Cleared
    /// before validation and populated only after successful checking. Retention
    /// copies parsed storage for this child; a later child can consume the result.
    pub prepared_source_output: Option<&'a mut Option<PreparedCheckedSource>>,
    /// Optional destination for post-evaluation discovery: the evaluated
    /// `Independent` provider selections this child's build authored and the
    /// usage of the evaluation that produced them. Cleared before validation
    /// and populated after build evaluation — before provider settlement's
    /// component-closure fence runs — so a caller that finds the compile
    /// rejected for an unanswered selection can publish the named components'
    /// descriptions and compile again with them attached. The discovery is
    /// reported whether the compile settles or rejects.
    pub independent_component_discovery_output: Option<&'a mut IndependentComponentDiscovery>,
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

impl<'a> CheckedCompileRequest<'a> {
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
            optimization_rollback: crate::compiler::checked::OptimizationRollback::default(),
            prepared_source_output: None,
            independent_component_discovery_output: None,
            restricted_build_grants: None,
            collect_timings: false,
            permit_unsettled_fused_service_fields: false,
        }
    }

    // Keep the caller's borrowed outputs on its thread; only owned compilation
    // inputs cross the compiler worker's static lifetime boundary.
    fn into_worker_request(
        self,
    ) -> (
        CheckedCompileRequest<'static>,
        Option<&'a mut Option<PreparedCheckedSource>>,
        Option<&'a mut IndependentComponentDiscovery>,
    ) {
        let source_output = self.prepared_source_output;
        let discovery_output = self.independent_component_discovery_output;
        (
            CheckedCompileRequest {
                root_path: self.root_path,
                build_execution_profile: self.build_execution_profile,
                target_name: self.target_name,
                package_inputs: self.package_inputs,
                build_dir: self.build_dir,
                filesystem_sponsor: self.filesystem_sponsor,
                evaluation_sponsor: self.evaluation_sponsor,
                build_snapshot: self.build_snapshot,
                optimization_rollback: self.optimization_rollback,
                restricted_build_grants: self.restricted_build_grants,
                collect_timings: self.collect_timings,
                prepared_source_output: None,
                independent_component_discovery_output: None,
                permit_unsettled_fused_service_fields: self.permit_unsettled_fused_service_fields,
            },
            source_output,
            discovery_output,
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
}

pub(crate) struct CheckedChildExecution<'a> {
    selected_target_profile: Option<target::TargetProfile>,
    /// `None` admits the compiler host without a catalogued profile.
    build_execution_profile: Option<target::TargetProfile>,
    package_inputs: Option<&'a PackageCompilationInputs>,
    build_dir: Option<&'a Path>,
    filesystem_sponsor: Option<crate::build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<crate::build_time_evaluation::BuildEvaluationSponsor>,
    build_snapshot: Option<crate::build_evaluation::BuildSnapshotRequest>,
    optimization_rollback: crate::compiler::checked::OptimizationRollback,
    /// Discovery write target owned by the compiling call: the evaluated
    /// `Independent` selections and evaluation usage are recorded between
    /// build evaluation and provider settlement so the roster survives a
    /// closure-fence rejection.
    independent_component_discovery: Option<&'a mut IndependentComponentDiscovery>,
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
        optimization_rollback: &crate::compiler::checked::OptimizationRollback,
        build_snapshot: Option<&crate::build_evaluation::BuildSnapshotRequest>,
    ) -> Self {
        let automatic_snapshot = package_inputs
            .filter(|inputs| inputs.canonical_source_metadata(inputs.root()).is_some())
            .map(|_| {
                crate::build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
            });
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
            independent_component_discovery: None,
            restricted_build_grants: None,
            permit_unsettled_fused_service_fields: false,
        }
    }

    pub(crate) fn package_inputs(&self) -> Option<&'a PackageCompilationInputs> {
        self.package_inputs
    }

    pub(crate) fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    pub(crate) fn optimization_rollback(&self) -> &crate::compiler::checked::OptimizationRollback {
        &self.optimization_rollback
    }

    pub(crate) fn permit_unsettled_fused_service_fields(&self) -> bool {
        self.permit_unsettled_fused_service_fields
    }
}

impl PreparedCheckedSource {
    /// Check another child from this immutable source frontier. Target attachments,
    /// build sponsors and output custody come exclusively from the new request.
    pub fn compile_to_checked(
        self,
        request: CheckedCompileRequest<'_>,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        compile_checked_request(request, Some(self))
    }

    fn compile_request(
        self,
        request: CheckedCompileRequest<'_>,
        independent_component_discovery: &mut IndependentComponentDiscovery,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        if request.root_path != self.root_path {
            return Err(vec![Diagnostic::error(
                "checked child compilation root does not match its prepared source checkpoint",
            )]);
        }
        let selected_target_profile = request
            .target_name
            .as_deref()
            .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
            .transpose()
            .map_err(|diagnostic| vec![diagnostic])?;
        self.compile_child(CheckedChildExecution {
            selected_target_profile,
            build_execution_profile: request
                .build_execution_profile
                .or_else(target::TargetProfile::host_if_supported),
            package_inputs: request.package_inputs.as_ref(),
            build_dir: request.build_dir.as_deref(),
            filesystem_sponsor: request.filesystem_sponsor,
            evaluation_sponsor: request.evaluation_sponsor,
            build_snapshot: request.build_snapshot,
            optimization_rollback: request.optimization_rollback,
            independent_component_discovery: Some(independent_component_discovery),
            restricted_build_grants: request.restricted_build_grants,
            permit_unsettled_fused_service_fields: request.permit_unsettled_fused_service_fields,
        })
    }

    pub fn prepare(
        root_path: &Path,
        package_sources: Option<
            std::sync::Arc<crate::package_compilation::PackageCompilationSourceInputs>,
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
            std::sync::Arc<crate::package_compilation::PackageCompilationSourceInputs>,
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
        })
    }

    fn compile_child(
        self,
        child: CheckedChildExecution<'_>,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let root_path = self.root_path.clone();
        self.admit_child(&child)?;
        let assembled = self.assemble(child.package_inputs)?;
        compile_assembled_checked_child(&root_path, child, assembled)
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
            if inputs.compilation_purpose() == crate::build_declarations::DependencyPurpose::Build
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

    /// Assemble the source set for package inputs an admitted child carries:
    /// the shared parsed sources plus the package imports and
    /// dependency-generated sources those inputs carry.
    pub(crate) fn assemble(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<AssembledSource, Vec<Diagnostic>> {
        let mut timings = self.shared_timings;
        let assembly_started = std::time::Instant::now();
        let (source_file_count, syntax) = self
            .source_checkpoint
            .assemble(package_inputs, &mut timings)?;
        timings.add_completed(
            crate::artifacts::compile_timings::SOURCE_ASSEMBLY,
            assembly_started.elapsed().as_micros(),
            crate::artifacts::allocations::AllocationDelta::default(),
        );
        Ok(AssembledSource {
            source_file_count,
            syntax,
            timings,
        })
    }
}

/// Compile source through checked Psi on the compiler worker stack.
/// Returns checked semantics and selected build evidence, without backend lowering
/// or native output. Build execution may stage generated sources in its admitted root.
pub fn compile_to_checked(
    request: CheckedCompileRequest<'_>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    compile_checked_request(request, None)
}

fn compile_checked_request(
    request: CheckedCompileRequest<'_>,
    prepared: Option<PreparedCheckedSource>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let (request, mut source_output, mut discovery_output) = request.into_worker_request();
    if let Some(output) = source_output.as_deref_mut() {
        *output = None;
    }
    if let Some(output) = discovery_output.as_deref_mut() {
        *output = IndependentComponentDiscovery::default();
    }
    let retain_source = source_output.is_some();
    // The discovery record is owned by the worker and returned beside the
    // compile verdict: a rejection loses only the sealed result, never the
    // evaluated selections the caller needs to publish named components.
    let (result, discovered) =
        crate::compiler::checked::checking::compile_thread::run_on_compile_thread(move || {
            let mut discovered = IndependentComponentDiscovery::default();
            let result = compile_checked_worker(request, prepared, retain_source, &mut discovered);
            Ok((result, discovered))
        })?;
    if let Some(output) = discovery_output {
        *output = discovered;
    }
    let (checked, retained_source) = result?;
    if let Some(output) = source_output {
        *output = retained_source;
    }
    Ok(checked)
}

/// The worker body of one checked compile. Discovery output is written
/// beside, never inside, the result: a component-closure rejection still
/// returns the evaluated `Independent` selections it recorded.
fn compile_checked_worker(
    request: CheckedCompileRequest<'static>,
    prepared: Option<PreparedCheckedSource>,
    retain_source: bool,
    independent_component_discovery: &mut IndependentComponentDiscovery,
) -> Result<(CheckedCompilation, Option<PreparedCheckedSource>), Vec<Diagnostic>> {
    // Validate target selection before loading source, including fresh requests.
    request
        .target_name
        .as_deref()
        .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let prepared = match prepared {
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
    let retained_source = retain_source.then(|| prepared.clone());
    let checked = prepared.compile_request(request, independent_component_discovery)?;
    Ok((checked, retained_source))
}

/// One target's assembled syntax, its physical source count and the timing
/// ladder the rest of its compile extends.
#[derive(Clone)]
pub(crate) struct AssembledSource {
    pub(crate) source_file_count: usize,
    pub(crate) syntax: crate::compiler::sources::AssembledSyntax,
    pub(crate) timings: CompileTimings,
}

fn compile_assembled_checked_child(
    root_path: &Path,
    mut child: CheckedChildExecution<'_>,
    assembled: AssembledSource,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let AssembledSource {
        source_file_count,
        syntax,
        mut timings,
    } = assembled;
    let selected_target_profile = child.selected_target_profile;
    let package_inputs = child.package_inputs;
    let optimization_rollback = child.optimization_rollback.clone();
    let independent_component_discovery = child.independent_component_discovery.take();
    let permit_unsettled_fused_service_fields = child.permit_unsettled_fused_service_fields;
    let build_started = std::time::Instant::now();
    let (built, sources) = build_continuation::evaluate_build_and_continue(
        root_path,
        child,
        source_file_count,
        syntax,
        &mut timings,
    )?;
    timings.add_completed(
        crate::artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
        build_started.elapsed().as_micros(),
        crate::artifacts::allocations::AllocationDelta::default(),
    );
    // The evaluated build configuration never leaves this crate, so the
    // discovery stop records `Independent` selections and the evaluation's
    // usage here — after the build computed them and before
    // `check_selected_execution` runs the component-closure fence that
    // rejects any of them left unanswered.
    if let Some(output) = independent_component_discovery {
        output.evaluation_usage = built.computed_build_config.evaluation_usage;
        output.selections.extend(
            built
                .computed_build_config
                .config
                .provider_selections
                .iter()
                .filter(|selection| {
                    selection.composition_mode
                        == crate::provider_planning::CompositionMode::Independent
                })
                .map(|selection| IndependentComponentSelection {
                    selection: selection.clone(),
                    component: selection.provider_type.package,
                }),
        );
    }
    let settlement_started = std::time::Instant::now();
    let execution = execution_settlement::check_selected_execution(
        built,
        selected_target_profile,
        package_inputs,
        &optimization_rollback,
        &mut timings,
        permit_unsettled_fused_service_fields,
    )?;
    timings.add_completed(
        crate::artifacts::compile_timings::EXECUTION_SETTLEMENT,
        settlement_started.elapsed().as_micros(),
        crate::artifacts::allocations::AllocationDelta::default(),
    );
    CheckedCompilation::seal(execution, sources, package_inputs, timings)
}
