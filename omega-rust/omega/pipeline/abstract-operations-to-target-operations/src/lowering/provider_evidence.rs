use super::shared::*;
use crate::AdmittedBoundaryExecution;

pub(super) fn bind_provider_executions(
    plan: &AbstractOperationPlan,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<Vec<BoundarySettlementBinding>, LoweringError> {
    settlements
        .iter()
        .map(|settlement| {
            let declaration = plan
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == settlement.boundary)
                .ok_or(LoweringError::UnknownBoundarySettlement(
                    settlement.boundary,
                ))?;
            let execution = match settlement.execution {
                AdmittedBoundaryExecution::Provider(provider_execution) => {
                    if provider_execution.requirement_identity() != declaration.identity {
                        return Err(LoweringError::ProviderExecutionRequirementMismatch {
                            boundary: settlement.boundary,
                            expected: declaration.identity.clone(),
                            actual: provider_execution.requirement_identity().to_owned(),
                        });
                    }
                    let provider_plan = target_operations::ProviderPlanReportIdentity::new(
                        provider_execution.provider_plan_report_identity(),
                    )
                    .ok_or_else(|| {
                        LoweringError::ProviderExecutionBinding("zero provider plan".into())
                    })?;
                    target_operations::ProviderExecutionBinding::from_execution_record(
                        provider_plan,
                        provider_execution.provider_execution_report_identity(),
                        provider_execution.provider_execution_report_fingerprint(),
                        provider_execution.normalized_root_report_identity(),
                        provider_execution.boundary_contract_report_fingerprint(),
                    )
                    .ok_or_else(|| {
                        LoweringError::ProviderExecutionBinding(
                            "admitted provider execution contains a zero identity".into(),
                        )
                    })?
                    .into()
                }
                AdmittedBoundaryExecution::CompilerBuiltin(execution) => {
                    target_operations::BoundaryExecutionBinding::CompilerBuiltin(execution)
                }
            };
            Ok(BoundarySettlementBinding {
                boundary: settlement.boundary,
                execution,
                realization: settlement.realization.clone(),
            })
        })
        .collect()
}
