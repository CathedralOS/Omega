//! Admission: fixing the exact entry, target inputs and authority of one
//! build occurrence into an opaque `AdmittedBuildProgram` before anything
//! runs.

use build_time_evaluation::{BuildEvaluationSponsor, PreparedBuildMachineProgram};
use diagnostics::Diagnostic;
use package_compilation::BuildMachineFilesystemScope;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_compilation::build_execution::{SelectedBuildMachine, admit_build_machine};
use symbols::SymbolHandle;

/// Private activation-local checkpoint. Entry selection and authority bind to
/// the prepared program; only the complete evaluation protocol can issue and
/// consume this value.
#[must_use = "an admitted build program must prepare its execution"]
pub(crate) struct AdmittedBuildProgram {
    pub(crate) prepared: PreparedBuildMachineProgram,
    pub(crate) providers: super::provider_execution::BuildProviderPlans,
    pub(crate) machine: SelectedBuildMachine,
    pub(crate) filesystem_scope: BuildMachineFilesystemScope,
    pub(crate) evaluation_sponsor: Option<BuildEvaluationSponsor>,
    pub(crate) execution_profile: Option<target::TargetProfile>,
    /// The validated `Build::artifact_only(builder)` application modifier. An
    /// artifact-only activation publishes retained outputs only: it may not
    /// bind executable roots or select boundary providers, and it must
    /// complete at least one required-output obligation.
    pub(crate) artifact_only: bool,
    pub(crate) default_source: bool,
    pub(crate) default_sources:
        Option<std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
}

impl AdmittedBuildProgram {
    /// The normalized restricted build-host requests this activation asks of
    /// the host, in issue order. They describe the admitted authority in
    /// package vocabulary — never the host paths or live handles it binds —
    /// so a consumer may retain accepted request meaning separately from the
    /// actual invocation grants.
    pub fn restricted_build_requests(&self) -> Vec<RestrictedBuildRequest> {
        self.filesystem_scope.restricted_build_requests(
            self.evaluation_sponsor.as_ref(),
            self.artifact_only,
            &self.machine.execution_mode,
        )
    }

    /// Fresh admission may identify an output-free Pure consumption candidate.
    /// This records immutable content only; it retains neither our entry token
    /// nor filesystem/evaluation grants. Restricted requests never qualify.
    pub(crate) fn pure_consumption_inputs(
        &self,
        packages: Option<&package_compilation::PackageCompilationInputs>,
        snapshot: Option<&package_compilation::BuildSnapshotRequest>,
        retained_check: Option<psi_compiler::RetainedCheck>,
    ) -> Option<crate::activation::completed_pure_build::CompletedPureBuildInputs> {
        if self.machine.filesystem_reachable
            || self.machine.execution_mode != build_time_evaluation::BuildMachineExecutionMode::Pure
            || !self.restricted_build_requests().is_empty()
        {
            eprintln!(
                "Pure Build input declined: entry {:?}, filesystem={}, pure={}, restricted_requests={}",
                self.machine.symbol,
                self.machine.filesystem_reachable,
                self.machine.execution_mode
                    == build_time_evaluation::BuildMachineExecutionMode::Pure,
                self.restricted_build_requests().len(),
            );
            return None;
        }
        let Some(checked) = self.prepared.checked_program() else {
            eprintln!(
                "Pure Build input declined: no checked program for {:?}",
                self.machine.symbol
            );
            return None;
        };
        crate::activation::completed_pure_build::CompletedPureBuildInputs::from_admitted(
            checked,
            retained_check,
            self.providers.clone(),
            self.machine.symbol,
            self.machine.initial_build.clone(),
            packages,
            snapshot,
            &self.filesystem_scope,
            self.artifact_only,
            self.default_source,
            crate::activation::measurement(),
            self.evaluation_sponsor.as_ref(),
        )
    }
}

/// Prepare and admit the program's exact build-machine activation.
///
/// Entry discovery, semantic checking and optional package authority have finished.
/// This stage binds that exact checked entry and admits its operational contract and
/// host grants without executing authored code. The returned carrier owns the
/// only entry token accepted by its prepared program.
pub(crate) fn admit_build_program(
    prepared: PreparedBuildMachineProgram,
    providers: super::provider_execution::BuildProviderPlans,
    machine_symbol: SymbolHandle,
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    execution_profile: Option<target::TargetProfile>,
    artifact_only: bool,
    source: package_compilation::BuildSourceSelection,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
    let typed = prepared.typed();
    if !typed
        .symbols
        .symbol_source_span(machine_symbol)
        .is_some_and(|span| span.source_id == source.source_id())
    {
        return Err(vec![Diagnostic::error(
            "Build entry differs from its source-owned selection",
        )]);
    }
    let operational_plan = validation::infer_operational_may(typed);
    let service_reach_plan = validation::infer_service_reaches(typed, &operational_plan);

    let mut filesystem_scope = filesystem_scope.clone();
    let machine = admit_build_machine(
        &prepared,
        machine_symbol,
        &service_reach_plan,
        &mut filesystem_scope,
    )?;
    Ok(AdmittedBuildProgram {
        prepared,
        providers,
        machine,
        filesystem_scope,
        evaluation_sponsor: evaluation_sponsor.cloned(),
        execution_profile,
        artifact_only,
        default_source: source.is_default(),
        default_sources: package_inputs
            .filter(|_| source.is_default())
            .map(package_compilation::PackageCompilationInputs::source_inputs),
    })
}
