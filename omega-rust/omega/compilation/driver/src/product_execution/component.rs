//! Publish a dependency's checked component for an independent consumer.

use crate::CheckedCompilation;
use crate::terminal::{
    ProducedTerminalApplication, TerminalProductRequest, TerminalProductionView,
    finish_terminal_application,
};
use artifacts::compile_timings::CompileTimings;
use compilation_products::IndependentComponentProduct;
use component_description::{
    ComponentDescriptionFacts, DescribeError, publish_component_description,
};
use diagnostics::Diagnostic;
use package_compilation::IndependentComponentDescription;

/// The compiler batches portable production; this owner retains only the
/// component's checked custody and its distinct publication policy. Review
/// descriptions do not become root reports or acquire new execution authority.
pub(crate) struct ComponentPublication {
    checked: CheckedCompilation,
    timings: CompileTimings,
    selections: optimization_core::OptimizationRollbackSettlement,
    exported_calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
}

impl ComponentPublication {
    pub(crate) fn prepare(mut checked: CheckedCompilation) -> Result<Self, Vec<Diagnostic>> {
        if checked.sources().package_identity().is_none() {
            return Err(vec![Diagnostic::error(
                "component publication requires package-aware checked custody for the dependency compiled as its own component",
            )]);
        }
        let selections = optimization_core::OptimizationRollback::default()
            .settle(&checked.build().config.settings.optimizations);
        // Export ABI planning belongs to this source-owned preparation, not
        // the native handoff after Terminal publication. Ordinary entry calls
        // do not enumerate every exported component requirement.
        let plans = checked
            .execution()
            .selected_provider_plan_facts
            .plans()
            .iter()
            .collect::<Vec<_>>();
        let exported_calling_applications = if plans.is_empty() {
            Vec::new()
        } else {
            let target = checked
                .execution()
                .selected_native_target()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "component native publication requires an exact selected target",
                    )]
                })?;
            provider_planning::calling_policy_plans::component_calling_applications(
                &checked.execution().program.typed,
                target,
                &plans,
                &checked.providers().boundary_calling_plan_realizations,
                &checked.build().config.opaque_representation_selections,
            )?
        };
        let timings = std::mem::take(checked.timings_mut());
        Ok(Self {
            checked,
            timings,
            selections,
            exported_calling_applications,
        })
    }

    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest {
            view: TerminalProductionView::checked(&self.checked, &mut self.timings),
            selections: self.selections.effective(),
        }
    }

    pub(crate) fn finish(
        self,
        produced: ProducedTerminalApplication,
    ) -> Result<IndependentComponentProduct, Vec<Diagnostic>> {
        let request = self.checked.terminal_native_proposal_request(
            self.selections.effective(),
            &self.exported_calling_applications,
        )?;
        let retained = finish_terminal_application(produced, request)?;
        let description = describe_component(&self.checked, &retained)?;
        let accepted_permissions =
            crate::native::accepted_terminal_authority_permission_policy(&self.checked)?;
        IndependentComponentProduct::new(description, retained, accepted_permissions)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
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
/// rather than from a caller assertion or from the description itself â€” the
/// package identity is the dependency's own checked package custody, and the
/// expected subject is the Terminal Psi identity observed on the module this
/// compilation produced.
///
/// Nothing here verifies the result. The description is unverified bytes
/// until the consuming build re-verifies them under its own admission
/// profile, and a root whose `Independent` selection reaches no published
/// description still rejects at the component-closure fence rather than
/// compiling as a fused edge.
fn describe_component(
    checked: &CheckedCompilation,
    retained: &compilation_products::RetainedTerminalArtifact,
) -> Result<IndependentComponentDescription, Vec<Diagnostic>> {
    let package = checked.sources().package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "component publication requires package-aware checked custody for the dependency compiled as its own component",
        )]
    })?;
    let artifact = retained.artifact();
    // A Psi component capsule has no native realization yet, so it publishes
    // no emitter-derived stack demand and no bound realization identity. The
    // description's own producer owns which rows that omits; nothing is
    // invented here to stand in for them. NativeComponentProduct publishes a
    // separate native description once image emission supplies those facts.
    let (expected_subject, description) = publish_component_description(ComponentDescriptionFacts {
        artifact,
        selected_provider_plans: &checked.execution().selected_provider_plan_facts,
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
