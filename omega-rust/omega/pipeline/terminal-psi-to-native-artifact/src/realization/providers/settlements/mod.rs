//! Optimizer module role: executable entrance. Provider-execution settlement: reject duplicate requirements,
//! admit each exact boundary realization, and publish executions in canonical order.

use std::collections::BTreeSet;

use crate::realization::model::{NativeRealizationCoreRequest, NativeRealizationInput};
use crate::realization::providers::AdmittedTerminalMechanism;
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;
use native_artifact::NativeProviderExecution;

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
    request: &NativeRealizationCoreRequest<'request>,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        Vec<NativeProviderExecution>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let mechanisms = validate_source_evaluated_import_coverage(
        input.plan(),
        request.selected_provider_plans,
        &request.terminal_authority_policy,
        request.target,
        request.external_binding_rows,
        request.settlements,
        request.native_callbacks,
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
    Ok((admitted, provider_executions, mechanisms))
}
