//! Optimizer module role: executable entrance. Provider-execution settlement: reject duplicate requirements,
//! admit each exact boundary realization, and publish executions in canonical order.

use std::collections::BTreeSet;

use crate::realization::model::{
    NativeBoundaryRealization, NativeProviderSettlement, NativeRealizationInput,
    NativeRealizationRequest,
};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_native_artifact::NativeProviderExecution;
use psi_diagnostics::Diagnostic;

mod boundary;
mod exact_plan;
mod normalized_foreign_call;

#[cfg(test)]
mod tests;

use boundary::settle_boundary;

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
    validate_source_evaluated_import_coverage(
        input.plan(),
        request.selected_provider_plans,
        request.settlements,
    )?;
    let mut seen_requirements = BTreeSet::new();
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());

    for settlement in request.settlements {
        let requirement = settlement.provider_execution.requirement_identity();
        if !seen_requirements.insert(requirement.to_owned()) {
            return Err(vec![Diagnostic::error(format!(
                "native realization received more than one provider execution for requirement `{requirement}`"
            ))]);
        }
        let (admitted_settlement, provider_execution) =
            settle_boundary(input, request, settlement)?;
        admitted.push(admitted_settlement);
        provider_executions.push(provider_execution);
    }

    provider_executions.sort_by(|left, right| {
        (
            left.requirement_identity(),
            left.provider_plan_report_identity(),
            left.provider_execution_report_identity(),
        )
            .cmp(&(
                right.requirement_identity(),
                right.provider_plan_report_identity(),
                right.provider_execution_report_identity(),
            ))
    });
    Ok((admitted, provider_executions))
}

fn validate_source_evaluated_import_coverage(
    plan: &omega_abstract_operations::AbstractOperationPlan,
    selected_plans: &omega_effects::SelectedProviderPlanFacts,
    settlements: &[NativeProviderSettlement<'_>],
) -> Result<(), Vec<Diagnostic>> {
    let boundary_identities = plan
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeSet::new();
    for operation in plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
    {
        let omega_abstract_operations::AbstractOperation::BoundaryCall { boundary, .. } = operation
        else {
            continue;
        };
        let Some(requirement) = boundary_identities.get(boundary) else {
            return Err(vec![Diagnostic::error(format!(
                "source-evaluated import demand cites absent boundary {boundary:?}"
            ))]);
        };
        demanded.insert(*requirement);
    }

    let mut required_imports = BTreeSet::new();
    for requirement in demanded {
        let import_matches = selected_plans
            .plans()
            .iter()
            .flat_map(|provider_plan| &provider_plan.rows)
            .filter(|row| {
                row.requirement_identity == requirement
                    && matches!(
                        row.binding,
                        omega_effects::provider_plan::ProviderBinding::Import { .. }
                    )
            })
            .count();
        match import_matches {
            0 => {}
            1 => {
                required_imports.insert(requirement);
            }
            count => {
                return Err(vec![Diagnostic::error(format!(
                    "demanded source-evaluated import `{requirement}` resolves to {count} selected import rows"
                ))]);
            }
        }
    }

    let mut covered_imports = BTreeSet::new();
    for settlement in settlements {
        let requirement = settlement.provider_execution.requirement_identity();
        let is_normalized = matches!(
            settlement.realization,
            NativeBoundaryRealization::NormalizedForeignCall(_)
        );
        if required_imports.contains(requirement) {
            if !is_normalized {
                return Err(vec![Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` requires a normalized foreign-call settlement"
                ))]);
            }
            if !covered_imports.insert(requirement) {
                return Err(vec![Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` received more than one normalized settlement"
                ))]);
            }
        } else if is_normalized {
            return Err(vec![Diagnostic::error(format!(
                "normalized foreign-call settlement for `{requirement}` does not cover a demanded selected import"
            ))]);
        }
    }
    if let Some(missing) = required_imports
        .iter()
        .find(|requirement| !covered_imports.contains(**requirement))
    {
        return Err(vec![Diagnostic::error(format!(
            "demanded source-evaluated import `{missing}` has no admitted native settlement"
        ))]);
    }
    Ok(())
}
