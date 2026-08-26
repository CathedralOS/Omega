use std::collections::BTreeSet;

use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, lower_to_target_operations_with_provider_executions,
};
use omega_terminal_installation_evidence::TerminalProviderExecutionEvidence;
use omega_terminal_target_operations::TerminalBoundaryRealization;
use psi_diagnostics::Diagnostic;

use super::CheckedCompilation;

#[derive(Debug)]
enum StagedAbstractOperations {
    Compatibility(omega_terminal_abstract_operations::TerminalAbstractOperationPlan),
    Optimized(Box<omega_lowering_optimizer::ValidatedOptimizedAbstractPlan>),
}

impl StagedAbstractOperations {
    fn plan(&self) -> &omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
        match self {
            Self::Compatibility(plan) => plan,
            Self::Optimized(optimized) => optimized.plan(),
        }
    }
}

/// One exact admitted provider execution selected for a staged terminal
/// component. This is an owned identity projection, not a provider occurrence
/// or an installation receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalComponentProviderExecution {
    requirement_identity: String,
    provider_plan: u64,
    provider_execution_identity: u64,
    provider_execution_fingerprint: u64,
    normalized_root_identity: u64,
    boundary_contract_fingerprint: u64,
}

impl TerminalComponentProviderExecution {
    fn from_evidence(evidence: &dyn TerminalProviderExecutionEvidence) -> Self {
        Self {
            requirement_identity: evidence.requirement_identity().to_owned(),
            provider_plan: evidence.provider_plan(),
            provider_execution_identity: evidence.provider_execution_identity(),
            provider_execution_fingerprint: evidence.provider_execution_fingerprint(),
            normalized_root_identity: evidence.normalized_root_identity(),
            boundary_contract_fingerprint: evidence.boundary_contract_fingerprint(),
        }
    }
}

impl TerminalProviderExecutionEvidence for TerminalComponentProviderExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    fn provider_plan(&self) -> u64 {
        self.provider_plan
    }

    fn provider_execution_identity(&self) -> u64 {
        self.provider_execution_identity
    }

    fn provider_execution_fingerprint(&self) -> u64 {
        self.provider_execution_fingerprint
    }

    fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_identity
    }

    fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

/// Provider-supplied realization input for one terminal boundary. The exact
/// requirement comes from the admitted execution rather than a caller-authored
/// numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct TerminalComponentProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn TerminalProviderExecutionEvidence,
    pub realization: TerminalBoundaryRealization,
}

/// A source-independent, non-visible terminal component candidate.
///
/// The candidate retains everything compilation can honestly establish. It
/// contains no output path, visibility receipt, installed-code claim, provider
/// occurrence, or progress-establishment receipt; those belong to deployment.
#[derive(Debug)]
pub struct TerminalComponentCandidate {
    target: omega_target::NativeTarget,
    entry_machine: String,
    semantic_bytes: Vec<u8>,
    proof_bytes: Vec<u8>,
    object: omega_terminal_image_emission::TerminalObjectArtifact,
    image: omega_terminal_image_emission::TerminalExecutableImage,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    provider_executions: Vec<TerminalComponentProviderExecution>,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
}

/// Complete owned compiler output transferred to deployment.
///
/// Only consuming a compiler-produced `TerminalComponentCandidate` yields
/// these parts. The parts themselves grant no installation or publication
/// authority; deployment must still bind them to real provider occurrences
/// and one exact `InstalledCode` occurrence.
#[derive(Debug)]
pub struct TerminalComponentCandidateParts {
    pub target: omega_target::NativeTarget,
    pub entry_machine: String,
    pub semantic_bytes: Vec<u8>,
    pub proof_bytes: Vec<u8>,
    pub object: omega_terminal_image_emission::TerminalObjectArtifact,
    pub image: omega_terminal_image_emission::TerminalExecutableImage,
    pub selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    pub provider_executions: Vec<TerminalComponentProviderExecution>,
    pub component_progress: Option<omega_effects::ComponentProgressManifest>,
}

impl TerminalComponentCandidate {
    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub fn entry_machine(&self) -> &str {
        &self.entry_machine
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        &self.semantic_bytes
    }

    pub fn proof_bytes(&self) -> &[u8] {
        &self.proof_bytes
    }

    pub const fn object(&self) -> &omega_terminal_image_emission::TerminalObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &omega_terminal_image_emission::TerminalExecutableImage {
        &self.image
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[TerminalComponentProviderExecution] {
        &self.provider_executions
    }

    pub const fn component_progress(&self) -> Option<&omega_effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    /// Transfer the complete non-visible compiler candidate into deployment.
    pub fn into_parts(self) -> TerminalComponentCandidateParts {
        TerminalComponentCandidateParts {
            target: self.target,
            entry_machine: self.entry_machine,
            semantic_bytes: self.semantic_bytes,
            proof_bytes: self.proof_bytes,
            object: self.object,
            image: self.image,
            selected_provider_plans: self.selected_provider_plans,
            provider_executions: self.provider_executions,
            component_progress: self.component_progress,
        }
    }
}

/// Lower one exact selected source entry into a canonical, non-visible terminal
/// component candidate. Runtime installation and publication deliberately do
/// not occur here.
pub fn stage_terminal_component(
    checked: &CheckedCompilation,
    target: omega_target::NativeTarget,
    subsystem: u16,
    profile: &psi_proof_admission::AdmissionProfile,
    settlements: &[TerminalComponentProviderSettlement<'_>],
) -> Result<TerminalComponentCandidate, Vec<Diagnostic>> {
    let selected_target = checked.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal component staging requires one exact selected native target",
        )]
    })?;
    if target != selected_target {
        return Err(vec![Diagnostic::error(format!(
            "terminal component staging target {target:?} does not match checked target {selected_target:?}"
        ))]);
    }
    let entry_machine = checked.selected_program_entry_machine().ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal component staging requires one exact selected program entry",
        )]
    })?;
    let lowered = psi_checked_trees_to_terminal::lower_machine(checked, entry_machine)
        .map_err(|error| stage_error("checked-to-terminal lowering", error))?;
    let semantic_bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .map_err(|error| stage_error("semantic encoding", error))?;
    let proof_bytes = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .map_err(|error| stage_error("proof encoding", error))?;
    let abstract_operations = if checked.optimization_selections().is_empty() {
        StagedAbstractOperations::Compatibility(
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections(
                &semantic_bytes,
                &proof_bytes,
                profile,
            )
            .map_err(|error| stage_error("verified artifact lowering", error))?,
        )
    } else {
        let request = omega_optimization_pipeline::compiler_baseline_request_v1(
            checked.optimization_selections(),
        )
        .expect("the selected staging branch is nonempty");
        StagedAbstractOperations::Optimized(Box::new(
            omega_optimization_pipeline::optimize_artifact_sections(
                &semantic_bytes,
                &proof_bytes,
                profile,
                request,
            )
            .map_err(|error| stage_error("verified optimization", error))?,
        ))
    };

    let mut seen_requirements = BTreeSet::new();
    let mut admitted = Vec::with_capacity(settlements.len());
    let mut provider_executions = Vec::with_capacity(settlements.len());
    for settlement in settlements {
        let evidence = settlement.provider_execution;
        let requirement = evidence.requirement_identity();
        if !seen_requirements.insert(requirement.to_owned()) {
            return Err(vec![Diagnostic::error(format!(
                "terminal component staging received more than one provider execution for requirement `{requirement}`"
            ))]);
        }
        let selected_plan = checked
            .selected_provider_plans()
            .plan_by_identity(evidence.provider_plan())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "terminal component provider execution for `{requirement}` names unselected plan {:#018x}",
                    evidence.provider_plan()
                ))]
            })?;
        if !selected_plan
            .rows
            .iter()
            .any(|row| row.requirement_identity == requirement)
        {
            return Err(vec![Diagnostic::error(format!(
                "terminal component provider execution for `{requirement}` is absent from selected plan `{}`",
                selected_plan.name
            ))]);
        }
        let matching_boundaries = abstract_operations
            .plan()
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == requirement)
            .collect::<Vec<_>>();
        let [boundary] = matching_boundaries.as_slice() else {
            return Err(vec![Diagnostic::error(match matching_boundaries.len() {
                0 => format!(
                    "terminal component provider execution cites absent requirement `{requirement}`"
                ),
                count => format!(
                    "terminal component requirement `{requirement}` resolves to {count} boundary declarations"
                ),
            })]);
        };
        admitted.push(AdmittedTerminalBoundarySettlement {
            boundary: boundary.id,
            provider_execution: evidence,
            realization: settlement.realization,
        });
        provider_executions.push(TerminalComponentProviderExecution::from_evidence(evidence));
    }
    provider_executions.sort_by(|left, right| {
        (
            left.requirement_identity.as_str(),
            left.provider_plan,
            left.provider_execution_identity,
        )
            .cmp(&(
                right.requirement_identity.as_str(),
                right.provider_plan,
                right.provider_execution_identity,
            ))
    });

    let target_operations = match abstract_operations {
        StagedAbstractOperations::Compatibility(abstract_operations) => {
            lower_to_target_operations_with_provider_executions(
                &abstract_operations,
                target,
                &admitted,
            )
            .map_err(|error| stage_error("target operation lowering", error))?
        }
        StagedAbstractOperations::Optimized(optimized) => {
            let _staged_assignment =
                omega_optimization_pipeline::stage_optimized_assignment_with_provider_executions(
                    *optimized, target, &admitted,
                )
                .map_err(|error| stage_error("optimized target assignment", error))?;
            return Err(
                crate::pipeline::optimization_gate::optimized_publication_unavailable(
                    checked.optimization_selections(),
                ),
            );
        }
    };
    let assigned =
        omega_terminal_target_operations_to_assigned_target_operations::assign_registers(
            &target_operations,
        )
        .map_err(|error| stage_error("register assignment", error))?;
    let machine_code = omega_terminal_machine_emission::emit_machine_code(&assigned)
        .map_err(|error| stage_error("machine-code emission", error))?;
    let object = omega_terminal_image_emission::build_terminal_object_artifact(&machine_code)
        .map_err(|error| stage_error("terminal object construction", error))?;
    let image = omega_terminal_image_emission::emit_terminal_executable_image(&object, subsystem)
        .map_err(|diagnostic| vec![diagnostic])?;

    Ok(TerminalComponentCandidate {
        target,
        entry_machine: entry_machine.to_owned(),
        semantic_bytes,
        proof_bytes,
        object,
        image,
        selected_provider_plans: checked.selected_provider_plans().clone(),
        provider_executions,
        component_progress: checked
            .component_progress()
            .filter(|manifest| !manifest.pending().is_empty())
            .cloned(),
    })
}

fn stage_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "terminal component {context} failed: {error}"
    ))]
}
