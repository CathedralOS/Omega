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
mod direct_foreign;
mod exact_plan;
mod normalized_foreign_call;
mod source_imports;
mod system_call;

use boundary::settle_boundary;
use source_imports::derive_source_evaluated_mechanisms;

pub(crate) fn settle_provider_executions<'request>(
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
    let callback_boundaries =
        source_imports::callback_boundaries(input.plan(), request.native_callbacks)?;
    let (mut mechanisms, cohort_rows) = derive_source_evaluated_mechanisms(
        input.plan(),
        request.selected_provider_plans,
        request.external_binding_rows,
        &callback_boundaries,
    )?;
    // Preparation settles one execution per selected row. Its retained family
    // and specialization relations may serve several original call contracts.
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());

    for settlement in request.settlements {
        let (admitted_settlement, provider_execution) =
            settle_boundary(input, request, settlement)?;
        admitted.extend(admitted_settlement);
        provider_executions.push(provider_execution);
    }

    let (direct, direct_mechanisms) = direct_foreign::settle(input, request, &callback_boundaries)?;
    if direct.iter().any(|direct| {
        admitted
            .iter()
            .any(|selected| selected.boundary == direct.boundary)
    }) {
        return Err(vec![Diagnostic::error(
            "one boundary retained both selected and direct foreign execution custody",
        )]);
    }
    admitted.extend(direct);
    mechanisms.extend(direct_mechanisms);

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
