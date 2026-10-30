//! Exercise canonical application admission after the actual source session is
//! released. Both retained and decoded artifacts must reproduce coverage and
//! reject a missing selection or selecting the requirement instead of its body.

use effects::provider_plan::ProviderBinding;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::{
    BoundaryApplicationSettlement, RetainedTerminalArtifact,
};

pub(super) fn inspect(retained: &RetainedTerminalArtifact) -> Result<(), String> {
    let proposal = retained
        .native_realization_proposal()
        .ok_or("boundary provider controls need a native proposal")?;
    let providers = proposal.providers();
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("boundary provider transport: {error:?}"))?;
    let plans = providers.selection.plans();
    let selected_index = plans.iter().position(|plan| {
        matches!(plan.rows.as_slice(), [row] if matches!(row.binding, ProviderBinding::CheckedAdapter { .. }))
            && loaded.semantic_module().boundary_application_demands.iter().any(|demand| {
                plan.schema.identity_text(plan.schema.trait_name) == demand.requirement().overload()
                    && plan.schema.trait_package_identity == demand.requirement().package()
            })
    }).ok_or("boundary provider controls require a demanded checked body")?;

    for artifact in [retained.artifact(), &loaded] {
        let settle = |selected: &effects::SelectedProviderPlanFacts,
                      executions: &[Vec<Option<effects::CompilerIntrinsicExecutionIdentity>>],
                      realizations: &[Vec<Option<String>>]| {
            BoundaryApplicationSettlement {
                selected_provider_plans: selected,
                selected_provider_provenance: selected.plans().iter()
                    .zip(executions).zip(realizations)
                    .map(|((plan, executions), realizations)| {
                        terminal_psi_to_abstract_operations::provider_admission::CompilerIntrinsicPlanProvenance {
                            plan,
                            semantic_schema: &plan.schema,
                            row_executions: executions,
                            row_numeric_realizations: realizations,
                        }
                    }).collect(),
                calling_applications: proposal.boundary_applications().calling.clone(),
            }.settle(artifact, selected)
        };
        let actual = settle(
            &providers.selection,
            &providers.intrinsic_executions,
            &providers.numeric_realizations,
        )
        .map_err(|errors| format!("boundary provider admission: {errors:?}"))?;
        if actual != proposal.boundary_applications().coverage {
            return Err("retained/decoded boundary admission changed exact coverage".into());
        }

        let mut missing = plans.to_vec();
        missing.remove(selected_index);
        let missing = effects::SelectedProviderPlanFacts::from_selected_plans(missing)?;
        let mut executions = providers.intrinsic_executions.clone();
        executions.remove(selected_index);
        let mut realizations = providers.numeric_realizations.clone();
        realizations.remove(selected_index);
        require_refusal(
            settle(&missing, &executions, &realizations),
            "Terminal boundary application has no unique exact selected row",
        )?;

        // This is the fixture's real public requirement, not a synthetic name.
        // Matching signatures cannot turn that bodyless declaration into the
        // exact checked realization selected by the source-authored plan.
        let mut wrong = plans.to_vec();
        let row = &mut wrong[selected_index].rows[0];
        if let ProviderBinding::CheckedAdapter {
            machine_identity, ..
        } = &mut row.binding
        {
            *machine_identity = row.requirement_identity;
        }
        let wrong = effects::SelectedProviderPlanFacts::from_selected_plans(wrong)?;
        require_refusal(
            settle(
                &wrong,
                &providers.intrinsic_executions,
                &providers.numeric_realizations,
            ),
            "selected boundary body has no unique checked Terminal candidate",
        )?;
    }
    Ok(())
}

fn require_refusal<T>(
    result: Result<T, Vec<diagnostics::Diagnostic>>,
    reason: &str,
) -> Result<(), String> {
    match result {
        Err(errors) if errors.iter().any(|error| error.message.contains(reason)) => Ok(()),
        Err(errors) => Err(format!(
            "boundary provider control missed {reason}: {errors:?}"
        )),
        Ok(_) => Err(format!(
            "boundary provider control accepted; expected {reason}"
        )),
    }
}
