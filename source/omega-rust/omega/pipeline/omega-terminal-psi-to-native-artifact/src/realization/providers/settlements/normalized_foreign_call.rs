use omega_target_operations::NormalizedForeignCallBinding;
use psi_diagnostics::Diagnostic;

pub(super) fn rejoin_normalized_foreign_call(
    selected_plan: &omega_effects::provider_plan::ProviderPlan,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    same_stack: &omega_task_plans::AdmittedSameStackContribution,
    provider_plan_report_identity: u64,
    requirement: &str,
    target: omega_target::NativeTarget,
) -> Result<NormalizedForeignCallBinding, Vec<Diagnostic>> {
    let selected_commitment = omega_task_plans::SameStackProviderPlanCommitment::from_digest(
        *selected_plan.identity_digest().as_bytes(),
    );
    if same_stack.provider_plan_report_identity() != provider_plan_report_identity
        || same_stack.provider_plan_commitment() != selected_commitment
        || same_stack.requirement_identity() != requirement
    {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not carry same-stack custody for the exact selected provider row"
        ))]);
    }
    let selected_rows = selected_plan
        .rows
        .iter()
        .filter(|row| row.requirement_identity == requirement)
        .collect::<Vec<_>>();
    let [selected_row] = selected_rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` resolves to {} selected provider rows",
            selected_rows.len()
        ))]);
    };
    let external_rows = external_binding_rows
        .iter()
        .filter(|row| row.requirement_identity == requirement)
        .collect::<Vec<_>>();
    let [external] = external_rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` resolves to {} retained external-binding rows",
            external_rows.len()
        ))]);
    };
    let (
        omega_effects::provider_plan::ProviderBinding::Import {
            locator: selected_locator,
        },
        omega_calling_conventions::ExternalBindingKind::Import {
            locator: retained_locator,
        },
        Some(boundary_entry_plan),
    ) = (
        &selected_row.binding,
        &external.binding,
        &external.boundary_entry_plan,
    )
    else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not rejoin an evaluated import row with a calling plan"
        ))]);
    };
    if selected_locator != retained_locator || retained_locator.target().native_target() != target {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not match the exact selected locator and native target"
        ))]);
    }
    Ok(NormalizedForeignCallBinding {
        locator: retained_locator.clone(),
        boundary_entry_plan: boundary_entry_plan.clone(),
        same_stack_contribution: same_stack.clone(),
    })
}
