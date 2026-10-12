use crate::AdmittedBoundaryExecution;
use crate::{AdmittedBoundarySettlement, LoweringError};
use abstract_operations::AbstractOperationPlan;
use target_operations::BoundarySettlementBinding;

pub(super) fn bind_provider_executions(
    plan: &AbstractOperationPlan,
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
                AdmittedBoundaryExecution::Provider(admitted_execution) => {
                    let provider_execution = admitted_execution.provider_execution();
                    if !admitted_execution.matches_public_boundary(
                        plan.psi,
                        declaration.id,
                        declaration.identity,
                    ) {
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
                AdmittedBoundaryExecution::DirectForeign(realization) => {
                    if plan.identities.text(declaration.identity)
                        != Some(realization.callable_identity())
                    {
                        return Err(LoweringError::ProviderExecutionBinding(
                            "direct foreign declaration identity changed".into(),
                        ));
                    }
                    target_operations::ProviderExecutionBinding::from_direct_foreign(
                        realization.identity(),
                    )
                    .ok_or_else(|| {
                        LoweringError::ProviderExecutionBinding(
                            "absent direct foreign commitment".into(),
                        )
                    })?
                    .into()
                }
                AdmittedBoundaryExecution::DirectCompilerIntrinsic(realization, execution) => {
                    if plan.identities.text(declaration.identity)
                        != Some(realization.callable_identity())
                    {
                        return Err(LoweringError::ProviderExecutionBinding(
                            "direct intrinsic declaration identity changed".into(),
                        ));
                    }
                    target_operations::BoundaryExecutionBinding::CompilerBuiltin(execution)
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
