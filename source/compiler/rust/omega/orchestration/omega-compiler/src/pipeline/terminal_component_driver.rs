use psi_diagnostics::Diagnostic;

use super::CheckedCompilation;
use super::build_config::{BuildEvaluationUsage, BuildObservationSummary};
use super::compile_options::CompileOptions;
use super::compile_report::CompileReport;
use super::output::{
    OwnedTerminalComponentDeploymentError, TerminalComponentDeploymentInputOwner,
    acquire_and_deploy_terminal_component_output,
};
use super::terminal_component_candidate::{
    TerminalComponentProviderSettlement, stage_terminal_component,
};

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
    profile: &'evidence psi_proof_admission::AdmissionProfile,
    settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
}

impl<'evidence> TerminalComponentStagingInputs<'evidence> {
    pub fn new(
        target: omega_target::NativeTarget,
        subsystem: u16,
        profile: &'evidence psi_proof_admission::AdmissionProfile,
        settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
    ) -> Self {
        Self {
            target,
            subsystem,
            profile,
            settlements,
        }
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub fn with_target(mut self, target: omega_target::NativeTarget) -> Self {
        self.target = target;
        self
    }

    pub const fn subsystem(&self) -> u16 {
        self.subsystem
    }

    pub const fn profile(&self) -> &psi_proof_admission::AdmissionProfile {
        self.profile
    }

    pub fn settlements(&self) -> &[TerminalComponentProviderSettlement<'evidence>] {
        &self.settlements
    }
}

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
/// This operation is deliberately downstream of the Psi-owned checked result.
/// It neither reruns the frontend nor grants runtime authority. Staging failure
/// returns both input owners; later failures preserve the established typed
/// acquisition/deployment custody.
pub fn stage_acquire_and_deploy_terminal_component_output<'evidence, Owner>(
    options: &CompileOptions,
    source_file_count: usize,
    checked: &CheckedCompilation,
    staging_inputs: TerminalComponentStagingInputs<'evidence>,
    deployment_owner: Owner,
    build_evaluation_usage: Option<BuildEvaluationUsage>,
    build_observation_summary: Option<BuildObservationSummary>,
) -> Result<CompileReport, Box<TerminalComponentDriverError<'evidence, Owner>>>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    let candidate = match stage_terminal_component(
        checked,
        staging_inputs.target,
        staging_inputs.subsystem,
        staging_inputs.profile,
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
