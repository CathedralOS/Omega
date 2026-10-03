use super::{
    exact_plan::selected_plan_from_exact_evidence,
    normalized_foreign_call::rejoin_normalized_foreign_call,
};
use crate::AdmittedBoundarySettlement;
use crate::provider_admission::{
    NativeBoundaryRealization, NativeProviderSettlement, NativeRealizationInput,
    ProviderAdmissionRequest,
};
use diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;

pub(super) fn settle_boundary<'request>(
    input: &NativeRealizationInput,
    request: &ProviderAdmissionRequest<'request>,
    settlement: &NativeProviderSettlement<'request>,
) -> Result<
    (
        AdmittedBoundarySettlement<'request>,
        &'request dyn ProviderExecutionEvidence,
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
        NativeBoundaryRealization::SystemCall => {
            target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                super::system_call::rejoin_system_call(
                    selected_plan,
                    request.external_binding_rows,
                    requirement,
                    request.target,
                )?,
            )
        }
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
        .filter(|boundary| {
            boundary.identity == requirement
                || request
                    .selected_provider_plans
                    .specialized_requirement_row(&boundary.identity)
                    .is_some_and(|specialization| {
                        specialization.selected_requirement_identity == requirement
                            && specialization.provider_plan_report_identity
                                == evidence.provider_plan_report_identity()
                    })
        })
        .collect::<Vec<_>>();
    let [boundary] = matching_boundaries.as_slice() else {
        return Err(vec![Diagnostic::error(match matching_boundaries.len() {
            0 => format!("native provider execution cites absent requirement `{requirement}`"),
            count => format!(
                "native requirement `{requirement}` resolves to {count} boundary declarations"
            ),
        })]);
    };
    if !super::super::compiler_builtins::boundary_contract_matches_selected_schema(
        boundary,
        selected_plan,
        requirement,
    ) {
        return Err(vec![Diagnostic::error(format!(
            "provider completion for `{requirement}` disagrees with its exact retained requirement contract"
        ))]);
    }
    if let target_operations::BoundarySettlementRealization::Builtin(builtin) = &realization {
        let completion = match builtin {
            target_operations::BoundaryRealization::HostedExitProcessI32(_) => {
                semantic_vocabulary::BoundaryCompletion::External(
                    semantic_vocabulary::ExternalCompletionKind::ProcessExit,
                )
            }
            target_operations::BoundaryRealization::MetadataOnlyPort(_)
            | target_operations::BoundaryRealization::DirectPortReadU8(_)
            | target_operations::BoundaryRealization::LinuxWriteLine(_)
            | target_operations::BoundaryRealization::HostedReadByte(_)
            | target_operations::BoundaryRealization::HostedWriteByteI32(_)
            | target_operations::BoundaryRealization::ClaimCompletionOnly(_)
            | target_operations::BoundaryRealization::NearestIeeeFloatArithmetic(_) => {
                semantic_vocabulary::BoundaryCompletion::NormalReturn
            }
        };
        if boundary.completion != completion {
            return Err(vec![Diagnostic::error(format!(
                "provider builtin for `{requirement}` contradicts its retained completion contract"
            ))]);
        }
    }
    Ok((
        AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: crate::AdmittedBoundaryExecution::Provider(evidence),
            realization,
        },
        evidence,
    ))
}
