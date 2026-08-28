#![forbid(unsafe_code)]

//! Canonical Terminal-Psi to target-native realization.
//!
//! This stage owns the target-dependent lowering join. It does not own source
//! compilation, component policy, executable installation, or publication.

use std::collections::BTreeSet;

use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, lower_to_target_operations_with_provider_executions,
};
use omega_terminal_installation_evidence::TerminalProviderExecutionEvidence;
pub use omega_terminal_native_artifact::{
    TerminalNativeArtifact, TerminalNativeArtifactParts, TerminalNativeProviderExecution,
    TerminalNativeSelectedProviderPlan,
};
use omega_terminal_target_operations::TerminalBoundaryRealization;
use psi_diagnostics::Diagnostic;

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

/// Provider-supplied realization input for one Terminal boundary. The exact
/// requirement comes from admitted execution evidence rather than a caller-
/// authored numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct TerminalNativeProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn TerminalProviderExecutionEvidence,
    pub realization: TerminalBoundaryRealization,
}

/// Realize one canonical Terminal-Psi artifact into an authority-free target
/// object and executable image. Ordinary native compilation and component
/// packaging share this exact source-independent operation.
pub fn realize_terminal_native_artifact(
    terminal_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    target: omega_target::NativeTarget,
    subsystem: u16,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    settlements: &[TerminalNativeProviderSettlement<'_>],
) -> Result<TerminalNativeArtifact, Vec<Diagnostic>> {
    terminal_artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let semantic_bytes = terminal_artifact.semantic_bytes();
    let proof_bytes = terminal_artifact.proof_bytes();
    let abstract_operations = if optimization_selections.is_empty() {
        StagedAbstractOperations::Compatibility(
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections(
                semantic_bytes,
                proof_bytes,
                profile,
            )
            .map_err(|error| realization_error("verified artifact lowering", error))?,
        )
    } else {
        let request =
            omega_optimization_pipeline::compiler_baseline_request_v1(optimization_selections)
                .expect("the selected realization branch is nonempty");
        StagedAbstractOperations::Optimized(Box::new(
            omega_optimization_pipeline::optimize_artifact_sections(
                semantic_bytes,
                proof_bytes,
                profile,
                request,
            )
            .map_err(|error| realization_error("verified optimization", error))?,
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
                "Terminal native realization received more than one provider execution for requirement `{requirement}`"
            ))]);
        }
        let selected_plan = selected_provider_plans
            .plan_by_identity(evidence.provider_plan())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "Terminal native provider execution for `{requirement}` names unselected plan {:#018x}",
                    evidence.provider_plan()
                ))]
            })?;
        if !selected_plan
            .rows
            .iter()
            .any(|row| row.requirement_identity == requirement)
        {
            return Err(vec![Diagnostic::error(format!(
                "Terminal native provider execution for `{requirement}` is absent from selected plan `{}`",
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
                    "Terminal native provider execution cites absent requirement `{requirement}`"
                ),
                count => format!(
                    "Terminal native requirement `{requirement}` resolves to {count} boundary declarations"
                ),
            })]);
        };
        admitted.push(AdmittedTerminalBoundarySettlement {
            boundary: boundary.id,
            provider_execution: evidence,
            realization: settlement.realization,
        });
        provider_executions.push(TerminalNativeProviderExecution::from_evidence(evidence));
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
            .map_err(|error| realization_error("target operation lowering", error))?
        }
        StagedAbstractOperations::Optimized(optimized) => {
            let _physical = omega_optimization_pipeline::
                stage_optimized_verified_physical_pipeline_with_provider_executions(
                    *optimized, target, &admitted,
                )
                .map_err(|error| {
                    optimized_physical_stage_error(optimization_selections, error)
                })?;
            return Err(optimized_publication_unavailable(optimization_selections));
        }
    };
    let assigned =
        omega_terminal_target_operations_to_assigned_target_operations::assign_registers(
            &target_operations,
        )
        .map_err(|error| realization_error("register assignment", error))?;
    let machine_code = omega_terminal_machine_emission::emit_machine_code(&assigned)
        .map_err(|error| realization_error("machine-code emission", error))?;
    let object = omega_terminal_image_emission::build_terminal_object_artifact(&machine_code)
        .map_err(|error| realization_error("terminal object construction", error))?;
    let image = omega_terminal_image_emission::emit_terminal_executable_image(&object, subsystem)
        .map_err(|diagnostic| vec![diagnostic])?;

    let mut selected_provider_plan_projections = selected_provider_plans
        .plans()
        .iter()
        .map(|plan| {
            TerminalNativeSelectedProviderPlan::new(
                plan.identity_fingerprint(),
                plan.rows
                    .iter()
                    .map(|row| row.requirement_identity.clone())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    selected_provider_plan_projections.sort_by_key(TerminalNativeSelectedProviderPlan::identity);
    TerminalNativeArtifact::from_replayed_parts(TerminalNativeArtifactParts {
        target,
        terminal_artifact,
        object,
        image,
        selected_provider_closure_identity: selected_provider_plans.normalized_identity(),
        selected_provider_plans: selected_provider_plan_projections,
        provider_executions,
    })
    .map_err(|error| realization_error("native artifact replay", error))
}

fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "Terminal native artifact {context} failed: {error}"
    ))]
}

fn optimized_physical_stage_error(
    selections: &omega_optimization_core::OptimizationSelections,
    error: impl std::fmt::Display,
) -> Vec<Diagnostic> {
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimizations `{names}` entered the optimized verified physical pipeline but failed at a named validation boundary: {error}; no output was installed"
    ))]
}

fn optimized_publication_unavailable(
    selections: &omega_optimization_core::OptimizationSelections,
) -> Vec<Diagnostic> {
    debug_assert!(!selections.is_empty());
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` completed the verified physical pipeline through post-allocation machine validation, but frame/exit, emission, artifact, and optimized component publication validation are not available yet; no output was installed",
        if selections.as_slice().len() == 1 { "" } else { "s" },
    ))]
}
