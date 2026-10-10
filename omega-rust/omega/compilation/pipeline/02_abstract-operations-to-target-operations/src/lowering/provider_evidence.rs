use crate::AdmittedBoundaryExecution;
use crate::{AdmittedBoundarySettlement, LoweringError};
use abstract_operations::AbstractOperationPlan;
use target_operations::BoundarySettlementBinding;

pub(super) fn bind_provider_executions(
    plan: &AbstractOperationPlan,
    verified_input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<Vec<BoundarySettlementBinding>, LoweringError> {
    settlements
        .iter()
        .map(|settlement| {
            let declaration =
                terminal_psi::find_boundary_machine(&plan.boundary_machines, settlement.boundary)
                    .ok_or(LoweringError::UnknownBoundarySettlement(
                    settlement.boundary,
                ))?;
            let execution = match settlement.execution {
                AdmittedBoundaryExecution::Provider(provider_execution) => {
                    if plan.identities.text(declaration.identity)
                        != Some(provider_execution.requirement_identity())
                        && !verified_input.is_some_and(|input| {
                            input.target_family_realizations().iter().any(|row| {
                                (row.head == settlement.boundary
                                    || row.member == settlement.boundary)
                                    && input
                                        .target_family_realization_identities()
                                        .text(row.selected_requirement_identity)
                                        == Some(provider_execution.requirement_identity())
                                    && row.provider_plan_report_identity
                                        == provider_execution.provider_plan_report_identity()
                            })
                        })
                    {
                        return Err(LoweringError::ProviderExecutionRequirementMismatch {
                            boundary: settlement.boundary,
                            expected: plan
                                .identities
                                .text(declaration.identity)
                                .unwrap_or_default()
                                .to_string(),
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
