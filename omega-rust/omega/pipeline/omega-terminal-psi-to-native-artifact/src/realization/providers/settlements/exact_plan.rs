use psi_diagnostics::Diagnostic;

pub(super) fn selected_plan_from_exact_evidence<'facts>(
    selected: &'facts omega_effects::SelectedProviderPlanFacts,
    report_identity: u64,
    exact_plan: &omega_effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> Result<&'facts omega_effects::provider_plan::ProviderPlan, Vec<Diagnostic>> {
    selected
        .plan_by_exact_evidence(report_identity, exact_plan)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "native provider execution for `{requirement}` does not carry exact evidence for selected plan {report_identity:#018x}"
            ))]
        })
}
