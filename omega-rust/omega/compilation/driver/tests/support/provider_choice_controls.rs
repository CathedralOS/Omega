//! Retain actual source-produced choice inputs, release source custody, then
//! select against both the retained and reloaded Terminal artifact. Compare
//! complete proposals, not display names or selected-row counts.

use std::path::Path;

use provider_planning::target_machines::{
    PreparedTargetProviderPublication, TargetProviderInventoryRequest,
    TargetProviderPreparationRequest,
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;

#[path = "provider_instance_controls.rs"]
mod provider_instance_controls;

pub(super) struct PreparedProviderChoices {
    publication: PreparedTargetProviderPublication,
    expected_instances: Vec<provider_instance_controls::ExpectedInstance>,
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
    let context = provider_planning::ProviderPlanningContext::new(
        &checked.execution().provider_source.program().typed,
        source::DependencyScope::Product,
    );
    let inventory = TargetProviderInventoryRequest {
        body_declarations: checked
            .execution()
            .provider_source
            .inventory()
            .direct_calls(),
        origins: checked.execution().provider_source.inventory().origins(),
        services: checked
            .execution()
            .provider_source
            .inventory()
            .services_for(checked.execution().provider_source.program())
            .map_err(|error| format!("provider choice source catalog: {error:?}"))?,
        defaults: checked.provider_defaults(),
        authored: checked.providers().authored_providers(),
        calling_plans: &checked.providers().boundary_calling_plan_realizations,
        bindings: &checked.providers().evaluated_via_bindings,
    };
    let targets = vec![(
        profile.target_name().to_owned(),
        inventory.prepare(profile.target_name()),
    )];
    let deployment = checked
        .build()
        .config
        .provider_deployment()
        .map_err(|errors| format!("provider choice deployment inputs: {errors:?}"))?;
    let publication = TargetProviderPreparationRequest {
        application_roots: deployment.roots.as_deref(),
        application_scopes: &deployment.scopes,
        context,
        exact_toolchain_sources: checked.sources().exact_toolchain_sources(),
        overrides: &checked.build().config.provider_selections,
        assumed_bindings: checked
            .execution()
            .provider_source
            .plans()
            .assumed_bindings(),
        independent_components: &checked.providers().verified_independent_components,
        selected: None,
        targets: &targets,
    }
    .prepare();
    let expected_instances = provider_instance_controls::prepare(&checked, root)?;
    drop(targets);
    drop(checked);
    drop(prepared);
    Ok(PreparedProviderChoices {
        publication,
        expected_instances,
    })
}

pub(super) fn inspect(
    prepared: PreparedProviderChoices,
    retained: &RetainedTerminalArtifact,
    target: &str,
) -> Result<(), String> {
    let PreparedProviderChoices {
        publication,
        expected_instances,
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
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("provider choice transport: {error:?}"))?;
    for artifact in [retained.artifact().view(), loaded.view()] {
        let actual = publication
            .clone()
            .publish(artifact, original.optimization_selection);
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
            let outcomes = publication
                .clone()
                .publish(omitted.view(), original.optimization_selection);
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
