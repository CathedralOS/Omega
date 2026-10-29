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

pub(super) fn prepare(
    root: &Path,
    profile: target::TargetProfile,
) -> Result<PreparedTargetProviderPublication, String> {
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
        &checked.execution().program.typed,
        source::DependencyScope::Product,
    );
    let inventory = TargetProviderInventoryRequest {
        context,
        defaults: checked.provider_defaults(),
        settlement: checked.providers(),
    };
    let targets = vec![(
        profile.target_name().to_owned(),
        inventory.prepare(profile.target_name()),
    )];
    let publication = TargetProviderPreparationRequest {
        context,
        exact_toolchain_sources: checked.sources().exact_toolchain_sources(),
        overrides: &checked.build().config.provider_selections,
        assumed_bindings: checked
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings(),
        targets: &targets,
    }
    .prepare();
    drop(targets);
    drop(checked);
    drop(prepared);
    Ok(publication)
}

pub(super) fn inspect(
    publication: PreparedTargetProviderPublication,
    retained: &RetainedTerminalArtifact,
    target: &str,
) -> Result<(), String> {
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
    }
    Ok(())
}
