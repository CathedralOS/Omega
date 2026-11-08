use diagnostics::Diagnostic;

/// Select by the actual declaration ABI, then authenticate authored policy
/// custody against the independently selected service schema.
pub(super) fn external_calling_application<'row>(
    row: &'row calling_conventions::ExternalBindingRow,
    plan: &effects::provider_plan::ProviderPlan,
    abi: &semantic_vocabulary::CallingAbiIdentity,
) -> Result<&'row calling_conventions::BoundaryCallingApplication, Vec<Diagnostic>> {
    let application = row.calling_application(abi).ok_or_else(|| {
        vec![Diagnostic::error(
            "external binding lost its exact declaration ABI application",
        )]
    })?;
    let dispatch_only = u32::from(matches!(
        row.binding,
        calling_conventions::ExternalBindingKind::TableFunction { .. }
    ));
    if application.dispatch_only_parameter_count != dispatch_only {
        return Err(vec![Diagnostic::error(
            "external calling application has another dispatch projection",
        )]);
    }
    let declared = plan
        .schema
        .calling_applications
        .iter()
        .any(|application| application.abi.requirement_identity == abi.requirement_identity);
    if declared {
        let expected = plan.schema.calling_application(abi).ok_or_else(|| {
            vec![Diagnostic::error(
                "selected external schema has no such exact declaration ABI application",
            )]
        })?;
        if application.commitment != expected.commitment.as_bytes()
            || application.report_fingerprint != expected.report_fingerprint
        {
            return Err(vec![Diagnostic::error(
                "external ABI application differs from its independently selected calling policy",
            )]);
        }
    }
    Ok(application)
}

pub(super) fn selected_plan_from_exact_evidence<'facts>(
    selected: &'facts effects::SelectedProviderPlanFacts,
    report_identity: u64,
    exact_plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> Result<&'facts effects::provider_plan::ProviderPlan, Vec<Diagnostic>> {
    selected
        .plan_by_exact_evidence(report_identity, exact_plan)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "native provider execution for `{requirement}` does not carry exact evidence for selected plan {report_identity:#018x}"
            ))]
        })
}
