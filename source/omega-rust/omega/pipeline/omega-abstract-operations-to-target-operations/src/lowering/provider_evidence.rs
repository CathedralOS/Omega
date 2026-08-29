use super::shared::*;

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
            if settlement.provider_execution.requirement_identity() != declaration.identity {
                return Err(LoweringError::ProviderExecutionRequirementMismatch {
                    boundary: settlement.boundary,
                    expected: declaration.identity.clone(),
                    actual: settlement
                        .provider_execution
                        .requirement_identity()
                        .to_owned(),
                });
            }
            let provider_plan = omega_target_operations::ProviderPlanIdentity::new(
                settlement.provider_execution.provider_plan(),
            )
            .ok_or_else(|| LoweringError::ProviderExecutionBinding("zero provider plan".into()))?;
            let provider_execution =
                omega_target_operations::ProviderExecutionBinding::from_execution_record(
                    provider_plan,
                    settlement.provider_execution.provider_execution_identity(),
                    settlement
                        .provider_execution
                        .provider_execution_fingerprint(),
                    settlement.provider_execution.normalized_root_identity(),
                    settlement
                        .provider_execution
                        .boundary_contract_fingerprint(),
                )
                .ok_or_else(|| {
                    LoweringError::ProviderExecutionBinding(
                        "admitted provider execution contains a zero identity".into(),
                    )
                })?;
            Ok(BoundarySettlementBinding {
                boundary: settlement.boundary,
                provider_execution,
                realization: settlement.realization,
            })
        })
        .collect()
}
