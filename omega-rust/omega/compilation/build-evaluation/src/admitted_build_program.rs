//! Admission: fixing the exact entry, target inputs and authority of one
//! build occurrence into an opaque `AdmittedBuildProgram` before anything
//! runs.

use crate::ComputedBuildConfig;
use crate::execute_admitted_build_program;
use build_time_evaluation::{BuildEvaluationSponsor, PreparedBuildMachineProgram};
use diagnostics::Diagnostic;
use package_compilation::BuildMachineFilesystemScope;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_compilation::build_execution::{SelectedBuildMachine, admit_build_machine};
use symbols::SymbolHandle;

/// Private activation-local checkpoint. Entry selection and authority bind to
/// the prepared program; only the complete evaluation protocol can issue and
/// consume this value.
#[must_use = "an admitted build program must be consumed by execute"]
pub(crate) struct AdmittedBuildProgram {
    pub(crate) prepared: PreparedBuildMachineProgram,
    pub(crate) machine: SelectedBuildMachine,
    pub(crate) filesystem_scope: BuildMachineFilesystemScope,
    pub(crate) evaluation_sponsor: Option<BuildEvaluationSponsor>,
    /// The validated `Build::artifact_only(builder)` application modifier. An
    /// artifact-only activation publishes retained outputs only: it may not
    /// bind executable roots or select boundary providers, and it must
    /// complete at least one required-output obligation.
    pub(crate) artifact_only: bool,
}

impl AdmittedBuildProgram {
    pub const fn selected_build_machine_symbol(&self) -> SymbolHandle {
        self.machine.symbol
    }

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
    pub fn pure_consumption_inputs(
        &self,
        packages: Option<&package_compilation::PackageCompilationInputs>,
        snapshot: Option<&package_compilation::BuildSnapshotRequest>,
    ) -> Option<crate::completed_pure_build::CompletedPureBuildInputs> {
        if self.machine.filesystem_reachable
            || self.machine.execution_mode != build_time_evaluation::BuildMachineExecutionMode::Pure
            || !self.restricted_build_requests().is_empty()
            || self.evaluation_sponsor.is_none()
        {
            return None;
        }
        crate::completed_pure_build::CompletedPureBuildInputs::from_admitted(
            self.prepared.checked_program()?,
            self.machine.symbol,
            self.machine.initial_build.clone(),
            packages?,
            snapshot,
            &self.filesystem_scope,
            self.artifact_only,
        )
    }

    /// Consume the exact admitted program through one evaluation.
    pub fn execute(self) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        execute_admitted_build_program(self)
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
    machine_symbol: SymbolHandle,
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    artifact_only: bool,
) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
    let typed = prepared.typed();
    let operational_plan = validation::infer_operational_may(typed);
    let service_reach_plan = validation::infer_service_reaches(typed, &operational_plan);

    let machine = admit_build_machine(
        &prepared,
        machine_symbol,
        &operational_plan,
        &service_reach_plan,
        filesystem_scope,
    )?;
    Ok(AdmittedBuildProgram {
        prepared,
        machine,
        filesystem_scope: filesystem_scope.clone(),
        evaluation_sponsor: evaluation_sponsor.cloned(),
        artifact_only,
    })
}
