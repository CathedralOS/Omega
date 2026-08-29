use std::collections::BTreeSet;

use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_native_artifact::NativeProviderExecution;
use psi_diagnostics::Diagnostic;

pub(crate) fn settle_provider_executions<'request>(
    input: &NativeRealizationInput,
    request: &NativeRealizationRequest<'request>,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        Vec<NativeProviderExecution>,
    ),
    Vec<Diagnostic>,
> {
    let mut seen_requirements = BTreeSet::new();
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());
    for settlement in request.settlements {
        let evidence = settlement.provider_execution;
        let requirement = evidence.requirement_identity();
        if !seen_requirements.insert(requirement.to_owned()) {
            return Err(vec![Diagnostic::error(format!(
                "native realization received more than one provider execution for requirement `{requirement}`"
            ))]);
        }
        let selected_plan = request
            .selected_provider_plans
            .plan_by_identity(evidence.provider_plan())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "native provider execution for `{requirement}` names unselected plan {:#018x}",
                    evidence.provider_plan()
                ))]
            })?;
        if !selected_plan
            .rows
            .iter()
            .any(|row| row.requirement_identity == requirement)
        {
            return Err(vec![Diagnostic::error(format!(
                "native provider execution for `{requirement}` is absent from selected plan `{}`",
                selected_plan.name
            ))]);
        }
        let matching_boundaries = input
            .plan()
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == requirement)
            .collect::<Vec<_>>();
        let [boundary] = matching_boundaries.as_slice() else {
            return Err(vec![Diagnostic::error(match matching_boundaries.len() {
                0 => format!("native provider execution cites absent requirement `{requirement}`"),
                count => format!(
                    "native requirement `{requirement}` resolves to {count} boundary declarations"
                ),
            })]);
        };
        admitted.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            provider_execution: evidence,
            realization: settlement.realization,
        });
        provider_executions.push(NativeProviderExecution::from_evidence(evidence));
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
    Ok((admitted, provider_executions))
}
