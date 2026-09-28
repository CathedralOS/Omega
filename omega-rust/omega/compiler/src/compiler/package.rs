//! Continue package-manager-owned checked production without reopening its inputs.
//!
//! Two products continue from one already-checked package: the retained
//! Terminal artifact, and — for a dependency compiled as its own component —
//! the canonical component description a consuming root attaches to its
//! target inputs.

use crate::CheckedCompilation;
use compilation_products::RetainedTerminalArtifact;
use component_description::{
    ComponentDescriptionFacts, DescribeError, publish_component_description,
};
use diagnostics::Diagnostic;
use package_compilation::IndependentComponentDescription;
use std::path::PathBuf;

/// Revalidate package custody before producing the retained Terminal artifact.
/// Final product requirements remain with the caller; this is an intermediate
/// artifact used by native production and independent component descriptions.
/// Package review owns the checked input; this does not rerun its build machine.
pub fn retained_terminal_artifact_from_checked_package(
    mut checked: CheckedCompilation,
    profile: proof_admission::AdmissionProfile,
    rollback: optimization_core::OptimizationRollback,
) -> Result<RetainedTerminalArtifact, Vec<Diagnostic>> {
    crate::checked::run_on_compile_thread(move || {
        checked.verify_current_source_consumption()?;
        if checked.production_subject()?.is_none() {
            return Err(vec![Diagnostic::error(
                "reviewed package Terminal production requires package-aware checked custody",
            )]);
        }
        let selections = rollback.settle(checked.optimization_selections());
        crate::terminal::produce_retained_terminal_artifact(
            &mut checked,
            &profile,
            selections.effective(),
        )
    })
}

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
pub fn published_independent_component_description(
    _root_path: PathBuf,
    checked: CheckedCompilation,
    profile: proof_admission::AdmissionProfile,
) -> Result<IndependentComponentDescription, Vec<Diagnostic>> {
    let package = checked.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "component publication requires package-aware checked custody for the dependency compiled as its own component",
        )]
    })?;
    // Read the component facts before the Terminal product consumes the
    // checked frontend; the description publishes exactly the selection this
    // compilation settled, not one recomputed beside it.
    let selected_provider_plans = checked.selected_provider_plans().clone();
    let component_progress = checked.component_progress().cloned();
    let retained = retained_terminal_artifact_from_checked_package(
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
