use psi_diagnostics::Diagnostic;

pub(super) struct NativeOptimizationAdmission<'checked> {
    pub(super) program_entry: &'checked omega_build_evaluation::SelectedCompilerProgramEntry,
    pub(super) target: omega_target::NativeTarget,
}

pub(super) fn reject_unconsumed_callbacks(
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let placements = checked.callback_placements();
    if placements.is_empty() {
        return Ok(());
    }
    let requirements = placements
        .iter()
        .map(|placement| format!("`{}`", placement.canonical_requirement_overload))
        .collect::<Vec<_>>()
        .join(", ");
    Err(vec![Diagnostic::error(format!(
        "native-artifact production cannot discard {} validated callback placement(s) for {requirements}; canonical Terminal callback-use custody is not implemented",
        placements.len()
    ))])
}

pub(super) fn admit(
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<NativeOptimizationAdmission<'_>, Vec<Diagnostic>> {
    let program_entry = checked.selected_program_entry().ok_or_else(|| {
        vec![Diagnostic::error(
            "native-artifact production requires one exact selected program entry",
        )]
    })?;
    let target = checked.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "native-artifact production requires one exact selected native target",
        )]
    })?;
    crate::pipeline::component_progress::reject_undischarged_build_bound_progress(
        checked.component_progress(),
    )?;
    omega_selected_dispatch::validate_selected_operator_terminal_custody(
        checked,
        checked.selected_provider_plans(),
    )?;
    omega_selected_dispatch::validate_fused_service_terminal_custody(
        checked,
        checked.selected_provider_provenance(),
    )?;
    Ok(NativeOptimizationAdmission {
        program_entry,
        target,
    })
}
