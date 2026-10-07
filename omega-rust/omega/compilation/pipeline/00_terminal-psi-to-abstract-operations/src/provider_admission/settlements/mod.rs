//! Optimizer module role: executable entrance. Provider-execution settlement:
//! derive each demanded leaf's mechanism, admit each exact boundary
//! realization, and publish executions in canonical order.

use crate::AdmittedBoundarySettlement;
use crate::provider_admission::AdmittedTerminalMechanism;
use crate::provider_admission::terminal_authority_policy::TerminalAuthorityPolicyRow;
use crate::provider_admission::{NativeRealizationInput, ProviderAdmissionRequest};
use diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;

mod boundary;
mod exact_plan;
mod normalized_foreign_call;
mod source_imports;
mod system_call;

use boundary::settle_boundary;
use source_imports::derive_source_evaluated_mechanisms;

pub fn settle_provider_executions<'request>(
    input: &NativeRealizationInput,
    request: &ProviderAdmissionRequest<'_, 'request>,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        Vec<&'request dyn ProviderExecutionEvidence>,
        Vec<AdmittedTerminalMechanism>,
        Vec<TerminalAuthorityPolicyRow>,
    ),
    Vec<Diagnostic>,
> {
    let (mechanisms, cohort_rows) = derive_source_evaluated_mechanisms(
        input.plan(),
        request.selected_provider_plans,
        request.external_binding_rows,
        request.native_callbacks,
    )?;
    // Preparation settled one execution per demanded requirement.
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());

    for settlement in request.settlements {
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
    Ok((admitted, provider_executions, mechanisms, cohort_rows))
}
