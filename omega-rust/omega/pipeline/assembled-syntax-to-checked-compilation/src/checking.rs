//! Prepare checked source, execute its build continuation, and seal selected execution.

mod build_continuation;
mod checked_compilation;
pub(crate) mod compile_thread;
mod const_evaluation;
mod execution_settlement;
pub(crate) mod phase_transitions;

pub use checked_compilation::CheckedCompilation;

use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use source_files_to_assembled_syntax::ImmutableSourceParseCheckpoint;
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
    pub optimization_rollback: crate::OptimizationRollback,
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
            optimization_rollback: crate::OptimizationRollback::default(),
            prepared_source_output: None,
            independent_component_discovery_output: None,
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
                prepared_source_output: None,
                independent_component_discovery_output: None,
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

struct CheckedChildExecution<'a> {
    selected_target_profile: Option<target::TargetProfile>,
    /// `None` admits the compiler host without a catalogued profile.
    build_execution_profile: Option<target::TargetProfile>,
    package_inputs: Option<&'a PackageCompilationInputs>,
    build_dir: Option<&'a Path>,
    filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    build_snapshot: Option<&'a build_evaluation::BuildSnapshotRequest>,
    optimization_rollback: crate::OptimizationRollback,
    /// Discovery write target owned by the compiling call: the evaluated
    /// `Independent` selections and evaluation usage are recorded between
    /// build evaluation and provider settlement so the roster survives a
    /// closure-fence rejection.
    independent_component_discovery: Option<&'a mut IndependentComponentDiscovery>,
}

impl CheckedChildExecution<'_> {
    #[cfg(test)]
    fn exact_target(selected_target_profile: target::TargetProfile) -> Self {
        Self {
            selected_target_profile: Some(selected_target_profile),
            build_execution_profile: target::TargetProfile::host_if_supported(),
            package_inputs: None,
            build_dir: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            build_snapshot: None,
            optimization_rollback: crate::OptimizationRollback::default(),
            independent_component_discovery: None,
        }
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
            build_snapshot: request.build_snapshot.as_ref(),
            optimization_rollback: request.optimization_rollback,
            independent_component_discovery: Some(independent_component_discovery),
        })
    }

    pub fn prepare(
        root_path: &Path,
        package_sources: Option<
            std::sync::Arc<package_compilation::PackageCompilationSourceInputs>,
        >,
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
        let mut shared_timings = CompileTimings::default();
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

    /// Check the prepared source for one target. `root_path` must be the
    /// root the checkpoint was prepared from; `build_dir` receives build
    /// evaluation output. An explicit snapshot supplies the caller's inventory
    /// and output requirements; otherwise canonical package custody selects
    /// its automatic full-inventory snapshot as before.
    pub fn check(
        self,
        root_path: &std::path::Path,
        target_name: Option<&str>,
        build_dir: std::path::PathBuf,
        package_inputs: Option<&PackageCompilationInputs>,
        optimization_rollback: &crate::OptimizationRollback,
        build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        if root_path != self.root_path {
            return Err(vec![Diagnostic::error(
                "checked child compilation root does not match its prepared source checkpoint",
            )]);
        }
        let selected_target_profile = target_name
            .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
            .transpose()
            .map_err(|diagnostic| vec![diagnostic])?;
        // A packaged root binding that carries the compiler-captured canonical
        // Source metadata index is sealed package custody. Every build
        // activation over such custody runs against a fresh private
        // materialization of the captured inventory with no invocation
        // roster, exactly what the package manager's review route requests,
        // so a retained Terminal production and the accepted review evidence
        // it must rejoin record the same build observation identity. A
        // binding without that index has no validated inventory to capture
        // against and keeps the live root it names.
        let automatic_snapshot = package_inputs
            .filter(|inputs| inputs.canonical_source_metadata(inputs.root()).is_some())
            .map(|_| build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()));
        self.compile_child(CheckedChildExecution {
            selected_target_profile,
            build_execution_profile: target::TargetProfile::host_if_supported(),
            package_inputs,
            build_dir: Some(&build_dir),
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            // The request narrows authority explicitly. Never replace it with
            // automatic package membership or silently fall back after failure.
            build_snapshot: build_snapshot.or(automatic_snapshot.as_ref()),
            optimization_rollback: optimization_rollback.clone(),
            independent_component_discovery: None,
        })
    }

    fn compile_child(
        self,
        child: CheckedChildExecution<'_>,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
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
        let target_name = child
            .selected_target_profile
            .map(target::TargetProfile::target_name);
        let mut timings = self.shared_timings;
        let (source_file_count, syntax) = match target_name {
            Some(target_name) => self
                .source_checkpoint
                .for_exact_target(target_name, child.package_inputs)?
                .assemble(&mut timings)?,
            None => self
                .source_checkpoint
                .assemble_targetless(child.package_inputs, &mut timings)?,
        };
        compile_assembled_checked_child(&self.root_path, child, source_file_count, syntax, timings)
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
    let (result, discovered) = crate::checking::compile_thread::run_on_compile_thread(move || {
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
        None => PreparedCheckedSource::prepare(
            &request.root_path,
            request
                .package_inputs
                .as_ref()
                .map(PackageCompilationInputs::source_inputs),
        )?,
    };
    let retained_source = retain_source.then(|| prepared.clone());
    let checked = prepared.compile_request(request, independent_component_discovery)?;
    Ok((checked, retained_source))
}

fn compile_assembled_checked_child(
    root_path: &Path,
    mut child: CheckedChildExecution<'_>,
    source_file_count: usize,
    syntax: source_files_to_assembled_syntax::AssembledSyntax,
    mut timings: CompileTimings,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let selected_target_profile = child.selected_target_profile;
    let package_inputs = child.package_inputs;
    let optimization_rollback = child.optimization_rollback.clone();
    let independent_component_discovery = child.independent_component_discovery.take();
    let (built, sources) = build_continuation::evaluate_build_and_continue(
        root_path,
        child,
        source_file_count,
        syntax,
        &mut timings,
    )?;
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
                    selection.composition_mode == provider_planning::CompositionMode::Independent
                })
                .map(|selection| IndependentComponentSelection {
                    selection: selection.clone(),
                    component: selection.provider_type.package,
                }),
        );
    }
    let execution = execution_settlement::check_selected_execution(
        built,
        selected_target_profile,
        package_inputs,
        &optimization_rollback,
        &mut timings,
    )?;
    CheckedCompilation::seal(execution, sources, package_inputs, timings)
}

#[cfg(test)]
mod continuation_tests;
#[cfg(test)]
mod execution_profile_tests;
