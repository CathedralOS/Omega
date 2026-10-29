//! Publish a dependency's checked component for an independent consumer.

use super::CompilationProductError;
use crate::CheckedCompilation;
use crate::terminal::{TerminalProductRequest, TerminalProductionView};
use artifacts::compile_timings::CompileTimings;
use compilation_products::IndependentComponentProduct;
use component_description::{
    ComponentDescriptionFacts, DescribeError, publish_component_description,
};
use diagnostics::Diagnostic;
use native_production::TerminalApplication;
use package_compilation::IndependentComponentDescription;

/// The compiler batches portable production; this owner retains only the
/// component's checked custody and its distinct publication policy. Review
/// descriptions do not become root reports or acquire new execution authority.
pub(crate) struct ComponentPublication {
    checked: CheckedCompilation,
    timings: CompileTimings,
    selections: optimization_core::OptimizationRollbackSettlement,
    exported_calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    retain_native_inputs: bool,
}

impl ComponentPublication {
    pub(crate) fn prepare(
        mut checked: CheckedCompilation,
        retain_native_inputs: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
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
            retain_native_inputs,
        })
    }

    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest {
            view: TerminalProductionView::checked(&self.checked, &mut self.timings),
            selections: self.selections.effective(),
            use_kind: crate::terminal::TerminalProductUse::Application,
        }
    }

    pub(crate) fn finish(
        self,
        produced: TerminalApplication,
    ) -> Result<IndependentComponentProduct, CompilationProductError> {
        let mut accounting = self.checked.source_completion_accounting().clone();
        // Application production retains the full declaration graph and every
        // evaluation entry, including obligations outside executable reach.
        // Complete those conditions before describing or caching the component;
        // a separate rootless production would repeat the same Psi work.
        let completed = self.complete_conditions(&produced, &mut accounting);
        if let Err(diagnostics) = completed {
            return Err(CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting,
            });
        }
        match self.publish(produced) {
            Ok(product) => Ok(product),
            Err(diagnostics) => Err(CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting,
            }),
        }
    }

    fn complete_conditions(
        &self,
        produced: &TerminalApplication,
        accounting: &mut build_time_evaluation::SourceCompletionAccounting,
    ) -> Result<(), Vec<Diagnostic>> {
        let execution = self.checked.execution();
        if execution.program.facts.proof_applications.has_obligations {
            let profile = execution
                .selected_target_profile
                .unwrap_or_else(target::TargetProfile::host);
            crate::terminal::scalar_completion::complete(
                &produced.artifact,
                profile.target_name(),
                &[],
                &produced.provider_proposals,
                accounting,
            )
            .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])?;
        }
        crate::execution_settlement::check_task_activations(execution)
    }

    fn publish(
        self,
        produced: TerminalApplication,
    ) -> Result<IndependentComponentProduct, Vec<Diagnostic>> {
        let request = self.checked.terminal_native_proposal_request(
            self.selections.effective(),
            &self.exported_calling_applications,
        )?;
        let accepted_permissions =
            super::native_inputs::accepted_terminal_authority_permission_policy(&self.checked)?;
        let native_inputs = self.retain_native_inputs.then(|| {
            native_production::retain_native_derivation_inputs(
                &produced.receipt,
                produced.physical_entry_adapter,
                &produced.provider_proposals,
                &request,
                &accepted_permissions,
            )
        });
        let retained = produced.prepare(request)?;
        let description = describe_component(&self.checked, &retained)?;
        let product = IndependentComponentProduct::new(description, retained, accepted_permissions)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(match native_inputs {
            Some(inputs) => product.with_native_derivation_inputs(inputs),
            None => product,
        })
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
