//! Admission: fixing the exact entry, target inputs and authority of one
//! build occurrence into an opaque `AdmittedBuildProgram` before anything
//! runs.

use crate::admission::configuration::BuildConfig;
use crate::execute_admitted_build_program;
use build_output::PackageGeneratedSource;
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildMachineExecutionMode, BuildTimeValue, PreparedBuildMachineProgram,
};
use diagnostics::Diagnostic;
use package_compilation::BuildMachineFilesystemScope;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_compilation::build_execution::{SelectedBuildMachine, admit_build_machine};
use package_compilation::{BuildEvaluationUsage, BuildObservationSummary};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedBuildConfig {
    pub config: BuildConfig,
    pub optimization_report_request: optimization_core::OptimizationReportRequest,
    pub evaluation_usage: Option<BuildEvaluationUsage>,
    pub observation_summary: Option<BuildObservationSummary>,
    pub selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    pub generated_sources: Vec<PackageGeneratedSource>,
}

/// Whether one admitted build activation has an executable build machine.
///
/// The selected entry itself is deliberately absent from this public
/// projection. It remains coupled to the prepared program inside
/// [`AdmittedBuildProgram`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmittedBuildProgramDisposition {
    NoBuildMachine,
    SelectedBuildMachine,
}

/// Compiler-owned authority decision retained by an admitted build program.
///
/// This is an audit projection of the exact interpreter mode held by the
/// checkpoint. It cannot be used to construct or replace that mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmittedBuildAuthorityVerdict {
    NoBuildMachine,
    Pure,
    Granted,
}

/// Opaque activation-local checkpoint for build-machine execution.
///
/// Preparation, exact entry selection, reach inference, target admission, and
/// authority validation all finish before this value is issued. The prepared
/// program and its entry token never leave the carrier independently, so a
/// caller cannot replace either half before execution.
///
/// The custody boundary is enforced by privacy, not a convention:
///
/// ```compile_fail
/// use build_evaluation::AdmittedBuildProgram;
/// use build_time_evaluation::PreparedBuildMachineProgram;
///
/// fn substitute_program(
///     admitted: &mut AdmittedBuildProgram,
///     foreign: PreparedBuildMachineProgram,
/// ) {
///     admitted.prepared = foreign;
/// }
/// ```
#[must_use = "an admitted build program must be consumed by execute"]
pub struct AdmittedBuildProgram {
    pub(crate) prepared: PreparedBuildMachineProgram,
    pub(crate) machine: Option<SelectedBuildMachine>,
    pub(crate) operational_plan: flow_effects::OperationalPlan,
    pub(crate) service_reach_plan: flow_effects::ServiceReachInferencePlan,
    pub(crate) filesystem_scope: BuildMachineFilesystemScope,
    pub(crate) evaluation_sponsor: Option<BuildEvaluationSponsor>,
    /// The validated `builder.artifact_only()` application modifier. An
    /// artifact-only activation publishes retained outputs only: it may not
    /// bind executable roots or select boundary providers, and it must
    /// complete at least one required-output obligation.
    pub(crate) artifact_only: bool,
}

impl AdmittedBuildProgram {
    pub fn disposition(&self) -> AdmittedBuildProgramDisposition {
        match &self.machine {
            None => AdmittedBuildProgramDisposition::NoBuildMachine,
            Some(_) => AdmittedBuildProgramDisposition::SelectedBuildMachine,
        }
    }

    pub fn authority_verdict(&self) -> AdmittedBuildAuthorityVerdict {
        match &self.machine {
            None => AdmittedBuildAuthorityVerdict::NoBuildMachine,
            Some(selected) => match &selected.execution_mode {
                BuildMachineExecutionMode::Pure => AdmittedBuildAuthorityVerdict::Pure,
                BuildMachineExecutionMode::Granted { .. } => AdmittedBuildAuthorityVerdict::Granted,
            },
        }
    }

    pub const fn selected_build_machine_symbol(&self) -> Option<SymbolHandle> {
        match &self.machine {
            None => None,
            Some(selected) => Some(selected.symbol),
        }
    }

    pub fn selected_build_machine_callable_identity(&self) -> Option<&str> {
        match &self.machine {
            None => None,
            Some(selected) => Some(&selected.normalized_callable_identity),
        }
    }

    pub const fn initial_build_snapshot(&self) -> Option<&BuildTimeValue> {
        match &self.machine {
            None => None,
            Some(selected) => Some(&selected.initial_build),
        }
    }

    pub const fn operational_plan(&self) -> &flow_effects::OperationalPlan {
        &self.operational_plan
    }

    pub const fn service_reach_plan(&self) -> &flow_effects::ServiceReachInferencePlan {
        &self.service_reach_plan
    }

    /// The normalized restricted build-host requests this activation asks of
    /// the host, in issue order. They describe the admitted authority in
    /// package vocabulary — never the host paths or live handles it binds —
    /// so a consumer may retain accepted request meaning separately from the
    /// actual invocation grants.
    pub fn restricted_build_requests(&self) -> Vec<RestrictedBuildRequest> {
        let Some(selected) = &self.machine else {
            return Vec::new();
        };
        self.filesystem_scope.restricted_build_requests(
            self.evaluation_sponsor.as_ref(),
            self.artifact_only,
            &selected.execution_mode,
        )
    }

    /// Consume the exact admitted program through one evaluation.
    pub fn execute(self) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        execute_admitted_build_program(self)
    }
}

/// Prepare and admit the program's exact build-machine activation.
///
/// This stage performs every selection and authority decision but executes no
/// authored code. The returned carrier owns the only entry token accepted by
/// its prepared program.
pub fn admit_build_program(
    typed: &TypedTrees,
    build_source_id: Option<source::SourceId>,
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    artifact_only: bool,
) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
    let prepared = PreparedBuildMachineProgram::prepare(typed)?;
    let typed = prepared.typed();
    let operational_plan = validation::infer_operational_may(typed);
    let service_reach_plan = validation::infer_service_reaches(typed, &operational_plan);

    let machine = admit_build_machine(
        &prepared,
        build_source_id,
        &operational_plan,
        &service_reach_plan,
        filesystem_scope,
    )?;
    Ok(AdmittedBuildProgram {
        prepared,
        machine,
        operational_plan,
        service_reach_plan,
        filesystem_scope: filesystem_scope.clone(),
        evaluation_sponsor: evaluation_sponsor.cloned(),
        artifact_only,
    })
}
