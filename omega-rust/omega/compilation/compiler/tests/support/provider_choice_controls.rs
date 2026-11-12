//! Retain actual source-produced choice inputs, release source custody, then
//! select against both the retained and reloaded Terminal artifact. Compare
//! complete proposals, not display names or selected-row counts.

use std::path::Path;

use provider_planning::target_machines::PreparedTargetProviderPublication;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;

use super::callback_body_controls;
#[path = "provider_instance_controls.rs"]
mod provider_instance_controls;

pub(super) struct PreparedProviderChoices {
    publication: PreparedTargetProviderPublication,
    selected_plans: Vec<effects::provider_plan::ProviderPlan>,
    expected_instances: Vec<provider_instance_controls::ExpectedInstance>,
    callback_bodies: callback_body_controls::PreparedCallbackBodies,
    callback_declarations: std::sync::Arc<psi_compiler::PreparedExternalProviderDeclarations>,
    calling_requirements: Vec<terminal_psi::CallbackRequirementId>,
}

pub(super) fn prepare(
    root: &Path,
    profile: target::TargetProfile,
) -> Result<PreparedProviderChoices, String> {
    let snapshot = crate::fixture_package_inputs::fixture_build_snapshot(root)
        .map_err(|errors| format!("provider choices build inputs: {errors:?}"))?;
    let discovered = crate::fixture_package_inputs::discover_fixture(
        root,
        package_compilation::BuildExecutionInputs {
            snapshot,
            ..Default::default()
        },
    )
    .map_err(|errors| format!("provider choices discovery: {errors:?}"))?;
    let mut prepared = crate::fixture_package_inputs::review_fixture(
        discovered,
        &[profile],
        compiler::CompilationProduct::Check,
    )
    .map_err(|errors| format!("provider choices admission: {errors:?}"))?;
    let checked = crate::fixture_package_inputs::check_fixture(
        &mut prepared,
        profile.target_name(),
        Default::default(),
    )
    .map_err(|errors| format!("provider choices checking: {errors:?}"))?;
    let selected_plans = checked.execution().plans().plans().to_vec();
    let publication = checked.providers().provider_publication.prepare(
        &[profile.target_name().to_owned()],
        None,
        checked.execution().plans().assumed_bindings(),
        &checked.providers().verified_independent_components,
    );
    let expected_instances = provider_instance_controls::prepare(&checked, root)?;
    let callback_bodies = callback_body_controls::prepare(&checked, root)?;
    let callback_declarations = std::sync::Arc::clone(&checked.execution().external_providers);
    let calling_requirements = checked
        .providers()
        .callback_calling_requirements(&callback_declarations);
    drop(checked);
    drop(prepared);
    Ok(PreparedProviderChoices {
        publication,
        selected_plans,
        expected_instances,
        callback_bodies,
        callback_declarations,
        calling_requirements,
    })
}

pub(super) fn inspect(
    prepared: PreparedProviderChoices,
    retained: &RetainedTerminalArtifact,
    target: &str,
) -> Result<(), String> {
    let PreparedProviderChoices {
        publication,
        selected_plans,
        expected_instances,
        callback_bodies,
        callback_declarations,
        calling_requirements,
    } = prepared;
    let original = retained
        .native_realization_proposal()
        .ok_or("provider choice controls need a native proposal")?
        .target_provider_proposals()
        .iter()
        .find(|proposal| proposal.target == target)
        .ok_or("provider choice controls lost the target proposal")?;
    original
        .selection
        .as_ref()
        .map_err(|errors| format!("provider choice original selection refused: {errors:?}"))?;
    let published_plans = &original
        .selection
        .as_ref()
        .map_err(|errors| format!("provider choice original selection refused: {errors:?}"))?
        .plans;
    if !selected_plans
        .iter()
        .eq(published_plans.iter().map(|row| &row.plan))
    {
        return Err(
            "checked provider choices differ from the complete published plan roster".into(),
        );
    }
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("provider choice transport: {error:?}"))?;
    for artifact in [retained.artifact().view(), loaded.view()] {
        let installation = original
            .selection
            .as_ref()
            .map_err(|errors| format!("provider choice selection: {errors:?}"))?
            .admit(artifact, target)?;
        if !installation.assumed_bindings().is_empty() {
            let scope = &artifact.module().scalar_applications.proof_scope;
            let entry = scope
                .callable_declaration(
                    artifact
                        .module()
                        .entry
                        .ok_or("assumed fixture has no runtime entry")?,
                )
                .ok_or("assumed runtime entry lost its checked declaration")?;
            let refusal = terminal_installation::admit_compiler_evaluation(
                artifact,
                target,
                entry,
                installation.clone(),
                semantic_vocabulary::compiler_evaluation::DeclarationCustody::Missing,
                None,
            )
            .err()
            .ok_or("runtime assumption authorized compiler evaluation")?;
            if !refusal.contains("assumed binding contracts are runtime-only") {
                return Err(format!(
                    "assumed compiler-evaluation control reached another refusal: {refusal}"
                ));
            }
            // Calling the independently checked supplier does not use the
            // assumed relationship. Inert runtime rows grant and forbid nothing.
            for binding in installation.assumed_bindings() {
                let supplier = scope
                    .callable_declaration(terminal_psi::TargetMachineCallable::Machine(
                        binding.implementation,
                    ))
                    .ok_or("assumed supplier lost its checked declaration")?;
                terminal_installation::admit_compiler_evaluation(
                    artifact,
                    target,
                    supplier,
                    installation.clone(),
                    semantic_vocabulary::compiler_evaluation::DeclarationCustody::Missing,
                    None,
                )
                .map_err(|reason| {
                    format!("inert assumption blocked its independently checked supplier: {reason}")
                })?;
            }
        }
        callback_bodies.inspect(artifact.module())?;
        let actual = publication.clone().publish(
            artifact,
            original.optimization_selection,
            if original.application_required {
                std::slice::from_ref(&original.target)
            } else {
                &[]
            },
            &callback_declarations,
            &calling_requirements,
        );
        // The retained producer now supplies the complete publication catalog.
        // This control still compares the exact target's whole proposal, as
        // the previous single-target Source reconstruction did.
        let actual = actual
            .into_iter()
            .filter(|proposal| proposal.target == target)
            .collect::<Vec<_>>();
        if actual.as_slice() != std::slice::from_ref(original) {
            return Err(format!(
                "source-free provider choice changed its complete proposal: {actual:?}"
            ));
        }
        let uncalled = provider_instance_controls::inspect(
            &expected_instances,
            artifact.module(),
            original
                .selection
                .as_ref()
                .map_err(|errors| format!("provider selection: {errors:?}"))?,
            target,
            original.application_roots.as_deref(),
        )?;
        if let Some(uncalled) = uncalled {
            // External Artifact ingress: remove only one uncalled binding,
            // retaining both its ordinary body and the independent obligation.
            // A nonempty candidate catalog must not hide this missing tuple.
            let mut omitted = artifact.module().clone();
            let before = omitted.provider_candidates.len();
            omitted
                .provider_candidates
                .retain(|candidate| candidate != &uncalled);
            if omitted.provider_candidates.len().checked_add(1) != Some(before) {
                return Err("provider instance control did not remove exactly one binding".into());
            }
            let optimization = terminal_codec::build_identity_optimization_execution_record(
                &omitted,
            )
            .map_err(|error| format!("missing-instance optimization envelope: {error:?}"))?;
            let omitted = CanonicalTerminalArtifact::from_parts(omitted, optimization, None)
                .map_err(|error| format!("missing-instance artifact: {error:?}"))?;
            let omitted = CanonicalTerminalArtifact::from_bytes(&omitted.to_bytes())
                .map_err(|error| format!("missing-instance external ingress: {error:?}"))?;
            let outcomes = publication.clone().publish(
                omitted.view(),
                original.optimization_selection,
                if original.application_required {
                    std::slice::from_ref(&original.target)
                } else {
                    &[]
                },
                &callback_declarations,
                &calling_requirements,
            );
            let outcomes = outcomes
                .into_iter()
                .filter(|proposal| proposal.target == target)
                .collect::<Vec<_>>();
            let [outcome] = outcomes.as_slice() else {
                return Err("missing-instance control lost its one target".into());
            };
            match &outcome.selection {
                Err(reasons)
                    if reasons.iter().any(|reason| {
                        reason.contains("retained closed requirement")
                            && reason.contains("0 canonical provider instances")
                    }) => {}
                other => {
                    return Err(format!(
                        "omitting an uncalled required tuple did not refuse at canonical coverage: {other:?}"
                    ));
                }
            }
        }
    }
    Ok(())
}
