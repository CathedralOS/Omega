// We publish a dependency's checked component (`ComponentPublication`) for an
// independent consumer. The compiler batches portable Terminal production across
// component requests; before publishing the canonical component description or
// admitting the product to the disk cache, we complete all scalar proof
// conditions and declaration task obligations (`complete_conditions`) and derive
// exported boundary calling applications from the produced Terminal artifact.

use super::CompilationProductError;
use crate::CheckedCompilation;
use crate::terminal::{TerminalProductRequest, TerminalProductionView};
use artifacts::compile_timings::CompileTimings;
use compilation_products::IndependentComponentProduct;
use component_description::{
    ComponentDescriptionFacts, DescribeError, publish_component_description,
};
use diagnostics::Diagnostic;
use native_compiler::TerminalApplication;
use package_compilation::IndependentComponentDescription;

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
        let exported_calling_applications = Vec::new();
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
        mut self,
        produced: TerminalApplication,
    ) -> Result<IndependentComponentProduct, CompilationProductError> {
        let mut accounting = self.checked.source_completion_accounting().clone();
        let completed = self.complete_conditions(&produced, &mut accounting);
        if let Err(diagnostics) = completed {
            return Err(CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting,
            });
        }
        self.exported_calling_applications = self
            .prepare_exported_calling_applications(&produced)
            .map_err(|diagnostics| CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting: accounting.clone(),
            })?;
        match self.publish(produced) {
            Ok(product) => Ok(product),
            Err(diagnostics) => Err(CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting,
            }),
        }
    }

    fn prepare_exported_calling_applications(
        &self,
        produced: &TerminalApplication,
    ) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, Vec<Diagnostic>> {
        let providers = self
            .checked
            .execution()
            .selected_provider_provenance
            .iter()
            .map(|provider| &provider.plan)
            .collect::<Vec<_>>();
        if providers.is_empty() {
            return Ok(Vec::new());
        }
        let target = self
            .checked
            .execution()
            .selected_target_profile
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "component native publication requires an exact selected target",
                )]
            })?;
        let calling = &self.checked.providers().calling_program;
        if calling.artifact.view().identity() != produced.artifact.view().identity() {
            return Err(vec![Diagnostic::error(
                "component calling catalog differs from the produced Terminal artifact",
            )]);
        }
        provider_planning::calling_policy_plans::component_calling_applications(
            target,
            calling,
            &providers,
            &self.checked.providers().boundary_calling_plan_realizations,
        )
    }

    fn complete_conditions(
        &self,
        produced: &TerminalApplication,
        accounting: &mut build_time_evaluation::SourceCompletionAccounting,
    ) -> Result<(), Vec<Diagnostic>> {
        let execution = self.checked.execution();
        if execution.has_proof_obligations {
            let profile = execution
                .selected_target_profile
                .unwrap_or_else(target::TargetProfile::host);
            build_settings::scalar_completion::complete(
                &produced.artifact,
                profile.target_name(),
                &[],
                &produced.provider_proposals,
                accounting,
            )
            .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])?;
        }
        crate::execution_settlement::check_terminal_task_activations(
            produced.artifact.view().module(),
            execution.plans(),
            execution.selected_target_profile,
        )
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
            super::accepted_terminal_authority_permission_policy(&self.checked)?;
        let native_inputs = self.retain_native_inputs.then(|| {
            native_compiler::retain_native_derivation_inputs(
                &produced.receipt,
                produced.physical_entry_adapter,
                &produced.provider_proposals,
                &request,
                &accepted_permissions,
                &[],
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
    let (expected_subject, description) = publish_component_description(ComponentDescriptionFacts {
        artifact,
        selected_provider_plans: checked.execution().plans(),
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
