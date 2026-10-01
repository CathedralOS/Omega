//! Publish a dependency's checked component for an independent consumer.

use compiler::CheckedCompilation;
use component_description::{
    ComponentDescriptionFacts, DescribeError, publish_component_description,
};
use diagnostics::Diagnostic;
use package_compilation::IndependentComponentDescription;

/// Publish the canonical component description of one dependency compiled as
/// its own component, ready to attach to a consuming root's target inputs
/// through `PackageCompilationInputs::with_independent_component_descriptions`.
///
/// This is the producer half of the verified-description contract
/// (wiki/spec/build/component_publication.md#verified-component-descriptions).
/// The description is published from the dependency's own retained Terminal
/// product: the embedded canonical artifact, the selected provider-plan facts
/// that compilation settled, and any build-bound progress manifest. Both
/// coordinates the carrier binds beside the bytes come from that compilation
/// rather than from a caller assertion or from the description itself — the
/// package identity is the dependency's own checked package custody, and the
/// expected subject is the Terminal Psi identity observed on the module this
/// compilation produced.
///
/// Nothing here verifies the result. The description is unverified bytes
/// until the consuming build re-verifies them under its own admission
/// profile, and a root whose `Independent` selection reaches no published
/// description still rejects at the component-closure fence rather than
/// compiling as a fused edge.
pub(super) fn publish_checked_component(
    checked: CheckedCompilation,
    profile: proof_admission::AdmissionProfile,
) -> Result<IndependentComponentDescription, Vec<Diagnostic>> {
    let package = checked.sources().package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "component publication requires package-aware checked custody for the dependency compiled as its own component",
        )]
    })?;
    // Read the component facts before the Terminal product consumes the
    // checked frontend; the description publishes exactly the selection this
    // compilation settled, not one recomputed beside it.
    let selected_provider_plans = checked.execution().selected_provider_plan_facts.clone();
    let component_progress = checked.execution().component_progress.as_ref().cloned();
    let (retained, _timings) = compiler::retained_terminal_artifact_from_checked_package(
        checked,
        profile,
        optimization_core::OptimizationRollback::default(),
    )?;
    let artifact = retained.artifact();
    // A Psi component capsule has no native realization yet, so it publishes
    // no emitter-derived stack demand and no bound realization identity. The
    // description's own producer owns which rows that omits; nothing is
    // invented here to stand in for them.
    let (expected_subject, description) = publish_component_description(ComponentDescriptionFacts {
        artifact,
        selected_provider_plans: &selected_provider_plans,
        component_progress: component_progress.as_ref(),
        stack_demand: None,
        realization_identity: None,
    })
    .map_err(|error| {
        vec![Diagnostic::error(match error {
            DescribeError::ArtifactIdentity(error) => format!(
                "published component module has no exact Terminal Psi identity: {error}"
            ),
            error => format!(
                "component description publication failed for the dependency compiled as its own component: {error}"
            ),
        })]
    })?;
    Ok(IndependentComponentDescription::new(
        package,
        expected_subject,
        description,
    ))
}
