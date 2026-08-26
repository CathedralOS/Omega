use std::collections::BTreeSet;

use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, lower_to_target_operations_with_provider_executions,
};
pub use omega_terminal_component_candidate::{
    TerminalComponentCandidate, TerminalComponentCandidateParts, TerminalComponentProviderExecution,
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

/// Provider-supplied realization input for one terminal boundary. The exact
/// requirement comes from the admitted execution rather than a caller-authored
/// numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct TerminalComponentProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn TerminalProviderExecutionEvidence,
    pub realization: TerminalBoundaryRealization,
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
            left.requirement_identity(),
            left.provider_plan(),
            left.provider_execution_identity(),
        )
            .cmp(&(
                right.requirement_identity(),
                right.provider_plan(),
                right.provider_execution_identity(),
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

    Ok(TerminalComponentCandidate::from_parts(
        TerminalComponentCandidateParts {
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
        },
    ))
}

fn stage_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "terminal component {context} failed: {error}"
    ))]
}
