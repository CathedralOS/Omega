use super::{
    exact_plan::selected_plan_from_exact_evidence,
    normalized_foreign_call::rejoin_normalized_foreign_call,
};
use crate::realization::model::{
    NativeBoundaryRealization, NativeProviderSettlement, NativeRealizationCoreRequest,
    NativeRealizationInput,
};
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use diagnostics::Diagnostic;
use native_artifact::NativeProviderExecution;

pub(super) fn settle_boundary<'request>(
    input: &NativeRealizationInput,
    request: &NativeRealizationCoreRequest<'request>,
    settlement: &NativeProviderSettlement<'request>,
) -> Result<
    (
        AdmittedBoundarySettlement<'request>,
        NativeProviderExecution,
    ),
    Vec<Diagnostic>,
> {
    let evidence = settlement.provider_execution;
    let requirement = evidence.requirement_identity();
    let selected_plan = selected_plan_from_exact_evidence(
        request.selected_provider_plans,
        evidence.provider_plan_report_identity(),
        settlement.provider_plan,
        requirement,
    )?;
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
    let realization = match settlement.realization {
        NativeBoundaryRealization::NormalizedForeignCall(same_stack) => {
            target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                rejoin_normalized_foreign_call(
                    selected_plan,
                    request.external_binding_rows,
                    same_stack,
                    evidence.provider_plan_report_identity(),
                    requirement,
                    request.target,
                )?,
            )
        }
        NativeBoundaryRealization::Builtin(realization) => {
            target_operations::BoundarySettlementRealization::Builtin(realization)
        }
    };
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
    Ok((
        AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution:
                abstract_operations_to_target_operations::AdmittedBoundaryExecution::Provider(
                    evidence,
                ),
            realization,
        },
        NativeProviderExecution::from_evidence(evidence),
    ))
}
