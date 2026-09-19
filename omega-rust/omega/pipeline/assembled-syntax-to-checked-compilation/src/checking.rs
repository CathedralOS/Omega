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

/// Inputs for checked-Psi compilation, without native publication authority.
/// All requests use the same source identity, package admission, replay, and
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
    /// Compiler-owned replay whose authored inputs and complete event stream must match.
    /// Replaying this record grants no host filesystem authority.
    pub replay_record: Option<build_evaluation::ReviewOnlyBuildFilesystemReplayRecord>,
    /// When present, the build occurrence executes against a captured
    /// immutable source snapshot and must complete each required output as a
    /// sealed regular file before its result may publish. Mutually exclusive
    /// with `replay_record`.
    pub build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    /// Release rollback subtracted from the authored optimization selection at
    /// each phase boundary this child executes. The authored selection remains
    /// the retained identity; the request can only remove exact rules it names.
    pub optimization_rollback: crate::OptimizationRollback,
    /// Optional destination for target-independent source preparation. Cleared
    /// before validation and populated only after successful checking. Retention
    /// copies parsed storage for this child; a later child can consume the result.
    pub prepared_source_output: Option<&'a mut Option<PreparedCheckedSource>>,
}

impl<'a> CheckedCompileRequest<'a> {
    /// Select source and optional target without package, sponsor, or replay inputs.
    pub fn new(root_path: &Path, target_name: Option<&str>) -> Self {
        Self {
            root_path: root_path.to_owned(),
            target_name: target_name.map(str::to_owned),
            build_execution_profile: None,
            package_inputs: None,
            build_dir: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            replay_record: None,
            build_snapshot: None,
            optimization_rollback: crate::OptimizationRollback::default(),
            prepared_source_output: None,
        }
    }

    // Keep the caller's borrowed output on its thread; only owned compilation
    // inputs cross the compiler worker's static lifetime boundary.
    fn into_worker_request(
        self,
    ) -> (
        CheckedCompileRequest<'static>,
        Option<&'a mut Option<PreparedCheckedSource>>,
    ) {
        let output = self.prepared_source_output;
        (
            CheckedCompileRequest {
                root_path: self.root_path,
                build_execution_profile: self.build_execution_profile,
                target_name: self.target_name,
                package_inputs: self.package_inputs,
                build_dir: self.build_dir,
                filesystem_sponsor: self.filesystem_sponsor,
                evaluation_sponsor: self.evaluation_sponsor,
                replay_record: self.replay_record,
                build_snapshot: self.build_snapshot,
                optimization_rollback: self.optimization_rollback,
                prepared_source_output: None,
            },
            output,
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
    replay_record: Option<&'a build_evaluation::ReviewOnlyBuildFilesystemReplayRecord>,
    build_snapshot: Option<&'a build_evaluation::BuildSnapshotRequest>,
    optimization_rollback: crate::OptimizationRollback,
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
            replay_record: None,
            build_snapshot: None,
            optimization_rollback: crate::OptimizationRollback::default(),
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
        self.compile_child_with_replay(CheckedChildExecution {
            selected_target_profile,
            build_execution_profile: request
                .build_execution_profile
                .or_else(target::TargetProfile::host_if_supported),
            package_inputs: request.package_inputs.as_ref(),
            build_dir: request.build_dir.as_deref(),
            filesystem_sponsor: request.filesystem_sponsor,
            evaluation_sponsor: request.evaluation_sponsor,
            replay_record: request.replay_record.as_ref(),
            build_snapshot: request.build_snapshot.as_ref(),
            optimization_rollback: request.optimization_rollback,
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
    /// evaluation output.
    pub fn check(
        self,
        root_path: &std::path::Path,
        target_name: Option<&str>,
        build_dir: std::path::PathBuf,
        package_inputs: Option<&PackageCompilationInputs>,
        optimization_rollback: &crate::OptimizationRollback,
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
        let build_snapshot = package_inputs
            .filter(|inputs| inputs.canonical_source_metadata(inputs.root()).is_some())
            .map(|_| build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()));
        self.compile_child_with_replay(CheckedChildExecution {
            selected_target_profile,
            build_execution_profile: target::TargetProfile::host_if_supported(),
            package_inputs,
            build_dir: Some(&build_dir),
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            replay_record: None,
            build_snapshot: build_snapshot.as_ref(),
            optimization_rollback: optimization_rollback.clone(),
        })
    }

    fn compile_child_with_replay(
        self,
        child: CheckedChildExecution<'_>,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        // Validate on every child, including prepared-source reuse: parsing is
        // shared, but generated build results belong to their execution profile.
        // This must precede assembly of the dependency's generated declarations.
        if let Some(inputs) = child.package_inputs {
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
    let (request, mut source_output) = request.into_worker_request();
    if let Some(output) = source_output.as_deref_mut() {
        *output = None;
    }
    let retain_source = source_output.is_some();
    let (checked, retained_source) =
        crate::checking::compile_thread::run_on_compile_thread(move || {
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
            let checked = prepared.compile_request(request)?;
            Ok((checked, retained_source))
        })?;
    if let Some(output) = source_output {
        *output = retained_source;
    }
    Ok(checked)
}

fn compile_assembled_checked_child(
    root_path: &Path,
    child: CheckedChildExecution<'_>,
    source_file_count: usize,
    syntax: source_files_to_assembled_syntax::AssembledSyntax,
    mut timings: CompileTimings,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let selected_target_profile = child.selected_target_profile;
    let package_inputs = child.package_inputs;
    let optimization_rollback = child.optimization_rollback.clone();
    let (built, sources) = build_continuation::evaluate_build_and_continue(
        root_path,
        child,
        source_file_count,
        syntax,
        &mut timings,
    )?;
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
