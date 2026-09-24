//! Rejoin each selected nearest fused multiply-add Terminal occurrence to its
//! exact selected plan. Both Terminal products use this one association: the
//! retained product carries the rows in its native-realization proposal and
//! the program-entry artifact carries them for the direct native route, so
//! neither route can realize the operation while dropping the provider custody
//! the checked program selected for it.

use assembled_syntax_to_checked_compilation::CheckedCompilation;
use diagnostics::Diagnostic;

pub(crate) fn associate(
    checked: &CheckedCompilation,
    native_target: target::NativeTarget,
    occurrences: &[lowered_psi::LoweredSelectedIeeeFloatFmaOccurrence],
) -> Result<Vec<compilation_report::TerminalIeeeFloatFmaOccurrenceProposal>, Vec<Diagnostic>> {
    occurrences
        .iter()
        .map(|occurrence| associate_one(checked, native_target, occurrence))
        .collect()
}

fn associate_one(
    checked: &CheckedCompilation,
    native_target: target::NativeTarget,
    occurrence: &lowered_psi::LoweredSelectedIeeeFloatFmaOccurrence,
) -> Result<compilation_report::TerminalIeeeFloatFmaOccurrenceProposal, Vec<Diagnostic>> {
    let fail = |message: String| {
        vec![Diagnostic::error(format!(
            "Terminal nearest-FMA operation {} {message}",
            occurrence.terminal_operation.get(),
        ))]
    };
    let matching_plan_indices = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .enumerate()
        .filter(|(_, plan)| {
            plan.report_fingerprint() == occurrence.provider_plan_report_fingerprint
                && plan.identity_digest().as_bytes()
                    == occurrence.provider_plan_commitment.as_bytes()
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [provider_plan_index] = matching_plan_indices.as_slice() else {
        return Err(fail(format!(
            "rejoins {} exact selected plans; expected one",
            matching_plan_indices.len(),
        )));
    };
    // The occurrence names the requirement it was selected for: a named
    // boundary operator or a top-level boundary requirement. The plan must
    // bind exactly that requirement's slot, whichever species spelled it.
    let plan = &checked.selected_provider_plans().plans()[*provider_plan_index];
    let requirement = provider_planning::IntrinsicRequirement::by_symbol(
        &checked.typed,
        occurrence.requirement_operator,
    )
    .ok_or_else(|| {
        fail("names no compiler-intrinsic requirement in the checked program".to_owned())
    })?;
    let ([method], [row]) = (plan.schema.methods.as_slice(), plan.rows.as_slice()) else {
        return Err(fail(format!(
            "selected plan `{}` must retain exactly one schema method and one row",
            plan.name,
        )));
    };
    if !requirement.plan_row_binds(plan, method, row) {
        return Err(fail(format!(
            "selected plan `{}` does not bind requirement `{}`",
            plan.name,
            requirement.display(),
        )));
    }
    let x86_admission = if native_target.architecture == target::Architecture::X86_64 {
        let Some(provider) = checked.x86_scalar_fma_provider() else {
            return Err(fail("lacks an admitted x86 deployment provider".to_owned()));
        };
        let matching = checked
            .x86_scalar_fma_plan_associations()
            .iter()
            .filter(|association| {
                association.matches_lowered_occurrence(
                    occurrence,
                    checked.selected_provider_plans(),
                    provider,
                )
            })
            .collect::<Vec<_>>();
        let [association] = matching.as_slice() else {
            return Err(fail(format!(
                "rejoins {} admitted x86 plan associations; expected one",
                matching.len(),
            )));
        };
        Some(compilation_report::TerminalX86ScalarFmaAdmission::new(
            association.slot(),
            association.admitted_provider(),
        ))
    } else {
        None
    };
    Ok(
        compilation_report::TerminalIeeeFloatFmaOccurrenceProposal::new(
            occurrence.terminal_operation,
            *provider_plan_index,
            occurrence.format,
            x86_admission,
        ),
    )
}
