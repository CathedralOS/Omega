//! Optimizer module role: executable entrance. Provider-execution settlement: reject duplicate requirements,
//! admit each exact boundary realization, and publish executions in canonical order.

use std::collections::BTreeSet;

use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_native_artifact::NativeProviderExecution;
use psi_diagnostics::Diagnostic;

mod boundary;
mod exact_plan;
mod normalized_foreign_call;
mod source_imports;

#[cfg(test)]
mod tests;

use boundary::settle_boundary;
use source_imports::validate_source_evaluated_import_coverage;

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
