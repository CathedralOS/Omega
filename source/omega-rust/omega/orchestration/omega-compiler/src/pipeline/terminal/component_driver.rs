use psi_diagnostics::Diagnostic;

use super::CheckedCompilation;
use super::build_config::{BuildEvaluationUsage, BuildObservationSummary};
use super::output::{
    OwnedTerminalComponentDeploymentError, TerminalComponentDeploymentInputOwner,
    acquire_and_deploy_terminal_component_output,
};
use super::terminal_component_candidate::{
    TerminalComponentProviderSettlement, stage_terminal_component,
};
use crate::compiler::CompileOptions;
use crate::compiler::CompileReport;

/// Concrete compiler-owned inputs for staging one authority-free terminal
/// component candidate.
///
/// Runtime installation and deployment authority remain outside this carrier.
/// The admission profile and provider settlements are borrowed from their real
/// owners and survive a rejected staging attempt unchanged.
#[derive(Debug)]
#[must_use = "terminal component staging inputs retain borrowed admission settlements"]
pub struct TerminalComponentStagingInputs<'evidence> {
    target: omega_target::NativeTarget,
    subsystem: u16,
    entry_machine: String,
    optimization_selections: omega_optimization_core::OptimizationSelections,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
    profile: &'evidence psi_proof_admission::AdmissionProfile,
    settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
}

impl<'evidence> TerminalComponentStagingInputs<'evidence> {
    /// Bind external staging policy to the exact target already selected by
    /// the owning checked result.
    ///
    /// This is a temporary cutover adapter, not another executable-semantics
    /// path: it projects only target identity. Terminal candidate staging
    /// remains the sole operation that consumes checked semantics.
    pub fn from_checked(
        checked: &CheckedCompilation,
        profile: &'evidence psi_proof_admission::AdmissionProfile,
        settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
    ) -> Result<Self, Box<TerminalComponentStagingInputBindingError<'evidence>>> {
        let subsystem = checked.subsystem();
        let Some(target) = checked.selected_native_target() else {
            return Err(Box::new(TerminalComponentStagingInputBindingError {
                subsystem,
                profile,
                settlements,
                diagnostic: Diagnostic::error(
                    "terminal component staging requires an exact native target selected by the owning checked result",
                ),
            }));
        };
        let Some(entry_machine) = checked.selected_program_entry_machine() else {
            return Err(Box::new(TerminalComponentStagingInputBindingError {
                subsystem,
                profile,
                settlements,
                diagnostic: Diagnostic::error(
                    "terminal component staging requires one exact selected program entry",
                ),
            }));
        };
        Ok(Self {
            target,
            subsystem,
            entry_machine: entry_machine.to_owned(),
            optimization_selections: checked.optimization_selections().clone(),
            selected_provider_plans: checked.selected_provider_plans().clone(),
            component_progress: checked.component_progress().cloned(),
            profile,
            settlements,
        })
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn subsystem(&self) -> u16 {
        self.subsystem
    }

    pub fn entry_machine(&self) -> &str {
        &self.entry_machine
    }

    pub const fn optimization_selections(
        &self,
    ) -> &omega_optimization_core::OptimizationSelections {
        &self.optimization_selections
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub const fn component_progress(&self) -> Option<&omega_effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    pub const fn profile(&self) -> &psi_proof_admission::AdmissionProfile {
        self.profile
    }

    pub fn settlements(&self) -> &[TerminalComponentProviderSettlement<'evidence>] {
        &self.settlements
    }
}

/// A targetless checked-result binding attempt retaining every external
/// staging input for retry against an exact selected target.
#[derive(Debug)]
#[must_use = "staging-input binding rejection retains admission settlements"]
pub struct TerminalComponentStagingInputBindingError<'evidence> {
    subsystem: u16,
    profile: &'evidence psi_proof_admission::AdmissionProfile,
    settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
    diagnostic: Diagnostic,
}

impl<'evidence> TerminalComponentStagingInputBindingError<'evidence> {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        u16,
        &'evidence psi_proof_admission::AdmissionProfile,
        Vec<TerminalComponentProviderSettlement<'evidence>>,
    ) {
        (self.subsystem, self.profile, self.settlements)
    }
}

impl std::fmt::Display for TerminalComponentStagingInputBindingError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for TerminalComponentStagingInputBindingError<'_> {}

/// A rejected complete terminal-component driver route.
#[derive(Debug)]
pub enum TerminalComponentDriverError<'evidence, Owner>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    Staging {
        diagnostics: Vec<Diagnostic>,
        staging_inputs: TerminalComponentStagingInputs<'evidence>,
        deployment_owner: Owner,
        source_file_count: usize,
        build_evaluation_usage: Option<BuildEvaluationUsage>,
        build_observation_summary: Option<BuildObservationSummary>,
    },
    Deployment(Box<OwnedTerminalComponentDeploymentError<Owner>>),
}

impl<Owner> std::fmt::Display for TerminalComponentDriverError<'_, Owner>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Staging { diagnostics, .. } => diagnostics
                .first()
                .map_or("terminal component staging failed", |error| {
                    error.message.as_str()
                })
                .fmt(formatter),
            Self::Deployment(error) => error.fmt(formatter),
        }
    }
}

impl<Owner> std::error::Error for TerminalComponentDriverError<'_, Owner> where
    Owner: TerminalComponentDeploymentInputOwner
{
}

/// Stage the ordinary checked result, acquire real deployment inputs from its
/// external owner, and retain the published result in a compiler report.
///
/// This operation is deliberately downstream of the current checked-result
/// cutover seam. `CheckedCompilation` remains a compatibility carrier until the
/// ordinary frontend directly hands off terminal Psi; this adapter must not
/// become a second checked-tree realization route. It neither reruns the
/// frontend nor grants runtime authority. Staging failure returns both input
/// owners; later failures preserve the established typed acquisition/deployment
/// custody.
pub fn stage_acquire_and_deploy_terminal_component_output<'evidence, Owner>(
    options: &CompileOptions,
    source_file_count: usize,
    checked: &CheckedCompilation,
    staging_inputs: TerminalComponentStagingInputs<'evidence>,
    deployment_owner: Owner,
) -> Result<CompileReport, Box<TerminalComponentDriverError<'evidence, Owner>>>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    let build_evaluation_usage = checked.build_evaluation_usage();
    let build_observation_summary = checked.build_observation_summary().cloned();
    let terminal_artifact = match psi_checked_trees_to_terminal::produce_terminal_artifact(
        checked,
        staging_inputs.entry_machine(),
    ) {
        Ok(artifact) => artifact,
        Err(error) => {
            return Err(Box::new(TerminalComponentDriverError::Staging {
                diagnostics: vec![Diagnostic::error(format!(
                    "terminal component artifact production failed: {error}"
                ))],
                staging_inputs,
                deployment_owner,
                source_file_count,
                build_evaluation_usage,
                build_observation_summary,
            }));
        }
    };
    let candidate = match stage_terminal_component(
        terminal_artifact,
        staging_inputs.entry_machine(),
        staging_inputs.target,
        staging_inputs.subsystem,
        staging_inputs.profile,
        staging_inputs.optimization_selections(),
        staging_inputs.selected_provider_plans(),
        staging_inputs.component_progress(),
        &staging_inputs.settlements,
    ) {
        Ok(candidate) => candidate,
        Err(diagnostics) => {
            return Err(Box::new(TerminalComponentDriverError::Staging {
                diagnostics,
                staging_inputs,
                deployment_owner,
                source_file_count,
                build_evaluation_usage,
                build_observation_summary,
            }));
        }
    };
    acquire_and_deploy_terminal_component_output(
        options,
        source_file_count,
        candidate,
        deployment_owner,
        build_evaluation_usage,
        build_observation_summary,
    )
    .map_err(|error| Box::new(TerminalComponentDriverError::Deployment(error)))
}
