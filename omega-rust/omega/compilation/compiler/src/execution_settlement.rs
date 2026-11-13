// Provider settlement chooses, for one target, the provider of every boundary
// requirement the Product's accepted Terminal module reaches
// (`provider_planning::target_supply`): a requirement with no declared supply
// or with competing supplies rejects the program, and a module that reaches
// none settles to no plan. The selected execution custody that carries
// settled plans on to products has no producer yet, so selection still ends
// in a refusal that says so.

use crate::checked::CheckedCompilation;
use crate::checked::PreparedProductTarget;
use artifacts::compile_timings::CompileTimings;
use compilation_products::{
    BuildAdmission, CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource,
    CompileFailure,
};
use diagnostics::Diagnostic;
use selected_dispatch::SelectedExecution;
use std::sync::Arc;

pub(crate) struct SelectedProviderCustody {
    pub(crate) terminal_publication: Arc<compilation_products::ProductTerminalPublication>,
    pub(crate) source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    pub(crate) execution: SelectedExecution,
    pub(crate) source_inspection: Arc<compilation_products::SelectedProviderSource>,
    pub(crate) settlement: provider_planning::ProviderSettlement,
    pub(crate) build: Arc<compilation_products::ComputedBuildConfig>,
    pub(crate) build_admission: BuildAdmission,
    pub(crate) provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub(crate) sources: Arc<package_compilation::CheckedSourceCustody>,
    pub(crate) timings: CompileTimings,
}

pub(crate) struct SelectedProductTarget {
    pub(crate) runtime_policy: Arc<package_evidence::permission_acceptance::PreparedRuntimePolicy>,
    pub(crate) custody: SelectedProviderCustody,
    pub(crate) independent_components:
        Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
}

impl SelectedProductTarget {
    pub(crate) fn select(
        _source: &CheckedProductSource,
        prepared: PreparedProductTarget,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Settlement rejects a demanded requirement with no supply or with
        // competing supplies, then forms the plans of the chosen ones. A
        // module that demands nothing settles to no plan.
        let PreparedProductTarget { profile, supply } = prepared;
        let settled = supply.settle()?;
        let plans = settled.plans()?;
        Err(vec![Diagnostic::error(format!(
            "no product route yet: provider settlement for `{}`: {} provider plans are settled, and the selected execution custody that carries them to products has no producer",
            profile
                .unwrap_or_else(target::TargetProfile::host)
                .target_name(),
            plans.len(),
        ))])
    }

    pub(crate) fn retain_independent_component_products(
        &mut self,
        products: &[Arc<compilation_products::IndependentComponentProduct>],
    ) -> Result<(), Vec<Diagnostic>> {
        let mut retained = Vec::new();
        let descriptions = &self.custody.settlement.independent_component_descriptions;
        let verified = &self.custody.settlement.verified_independent_components;
        if descriptions.len() != verified.len() {
            return Err(vec![Diagnostic::error(
                "independent component descriptions lost their consumer verdict roster",
            )]);
        }
        let applications = self
            .custody
            .settlement
            .calling_applications(&[])
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for (description, component) in descriptions.iter().zip(verified) {
            let mut matching = products.iter().filter(|product| {
                product.description() == description
                    && product
                        .retained()
                        .native_realization_proposal()
                        .is_some_and(|proposal| {
                            Some(proposal.target_profile())
                                == self.custody.execution.selected_target_profile
                        })
            });
            let Some(product) = matching.next() else {
                continue;
            };
            if matching.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "independent component description has competing retained native products",
                )]);
            }
            if component.subject() != description.expected_subject() {
                return Err(vec![Diagnostic::error(
                    "independent component verdict belongs to another description subject",
                )]);
            }
            retained.push(Arc::clone(product));
        }

        let execution = &self.custody.execution;
        let facts = execution.plans();
        let mut receiving_identities = None;
        let mut selected_components = Vec::new();
        let mut unavailable = None;
        for selected in &execution.selected_provider_provenance {
            if selected
                .selected_by
                .composition_mode()
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                != provider_planning::CompositionMode::Independent
            {
                continue;
            }
            let declarations =
                match &execution.native_provider_sources {
                    Ok(declarations) => declarations,
                    Err(refused) => {
                        unavailable.get_or_insert_with(|| format!(
                        "component call has no prepared native declaration roster: {refused:?}"
                    ));
                        continue;
                    }
                };
            let selected_inputs = (|| {
                let mut declarations = declarations
                    .declarations
                    .iter()
                    .filter(|declaration| declaration.plan == selected.plan);
                let declaration = declarations
                    .next()
                    .ok_or("component call lost its exact selecting declaration")?;
                if declarations.next().is_some() {
                    return Err("component call has competing exact selecting declarations");
                }
                let owner = declaration
                    .provider_declaration
                    .as_deref()
                    .ok_or("component call has no selecting owner")?;
                let mut components = verified.iter().filter(|component| {
                    component
                        .realizes_selected_plan(&selected.plan, owner)
                        .is_ok()
                });
                let component = components
                    .next()
                    .ok_or("independent native production has no exact verified component")?;
                if components.next().is_some() {
                    return Err("independent native production has competing component verdicts");
                }
                let mut products = retained.iter().filter(|product| {
                    product.description().expected_subject() == component.subject()
                });
                let product = products.next()
                    .ok_or("independent native production requires exactly one complete retained producing product")?;
                if products.next().is_some() {
                    return Err(
                        "independent native production requires exactly one complete retained producing product",
                    );
                }
                let digest = selected.plan.identity_digest();
                let mut plans = facts
                    .plans()
                    .iter()
                    .filter(|plan| plan.identity_digest() == digest);
                let plan = plans
                    .next()
                    .ok_or("component call lost its complete selected provider plan")?;
                if plans.next().is_some() {
                    return Err("component call has competing complete provider plans");
                }
                Ok((component, product, plan, owner))
            })();
            let (component, product, plan, owner) = match selected_inputs {
                Ok(inputs) => inputs,
                Err(reason) => {
                    unavailable.get_or_insert_with(|| reason.to_owned());
                    continue;
                }
            };
            let component = compilation_products::SelectedIndependentComponent::join(
                Arc::clone(component),
                product,
                plan.clone(),
                owner,
                Arc::clone(
                    receiving_identities
                        .get_or_insert_with(|| Arc::new(facts.identities().clone())),
                ),
                &applications,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "independent component native inputs: {reason}"
                ))]
            })?;
            selected_components.push(component);
        }
        self.independent_components = match unavailable {
            Some(reason) => Err(reason),
            None => Ok(selected_components),
        };
        Ok(())
    }

    pub(crate) fn finish_check(mut self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let terminal = crate::terminal::check_terminal_conditions(&mut self.custody)?;
        self.check_declaration_requirements(terminal.as_ref().map(|product| product.module()))?;
        self.into_checked_compilation()
    }

    pub(crate) fn check_declaration_requirements(
        &self,
        terminal: Option<&terminal_psi::TerminalModule>,
    ) -> Result<(), Vec<Diagnostic>> {
        let execution = &self.custody.execution;
        let module = terminal.ok_or_else(|| {
            vec![Diagnostic::error(
                "no product route yet: declaration requirements have no accepted Terminal module",
            )]
        })?;
        check_terminal_task_activations(
            module,
            execution.plans(),
            execution.selected_target_profile,
        )
    }

    pub(crate) fn into_checked_compilation(self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        checked_compilation(self.custody, self.independent_components)
    }
}

pub(crate) fn check_terminal_task_activations(
    module: &terminal_psi::TerminalModule,
    providers: &effects::SelectedProviderPlanFacts,
    profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    task_plans::realize_task_activations(
        &module.task_activation_demands,
        &module.identities,
        providers,
        profile.unwrap_or_else(target::TargetProfile::host),
    )?;
    Ok(())
}

pub(crate) struct SelectedCompileOutcome {
    pub(crate) result: Result<SelectedProductTarget, CompileFailure>,
    pub(crate) independent_components: compilation_products::IndependentComponentDiscovery,
    // Separately selected test roots over the same checked source, in roster
    // order (wiki/spec/build/testing.md). Empty when no enabled group exists.
    pub(crate) tests: Vec<(
        compilation_products::TestRootIdentity,
        Result<SelectedProductTarget, CompileFailure>,
    )>,
    pub(crate) test_notes: Vec<compilation_products::TestRosterEntry>,
}

impl SelectedCompileOutcome {
    pub(crate) fn finish_check(self) -> CheckedCompileOutcome {
        CheckedCompileOutcome {
            result: self
                .result
                .and_then(|selected| selected.finish_check().map_err(CompileFailure::from)),
            independent_components: self.independent_components,
        }
    }
}

fn checked_compilation(
    selection: SelectedProviderCustody,
    independent_components: Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let SelectedProviderCustody {
        terminal_publication,
        source_completion_accounting,
        execution,
        source_inspection,
        settlement,
        build,
        build_admission,
        provider_defaults,
        sources,
        timings,
    } = selection;
    CheckedCompilation::from_settled_execution(
        CheckedCompilationInputs {
            settled: execution,
            source_inspection,
            build,
            build_admission,
            provider_defaults,
            providers: settlement,
            independent_components,
        },
        sources,
        timings,
        terminal_publication,
    )
    .map(|checked| checked.with_source_completion_accounting(source_completion_accounting))
}

pub(crate) fn select_product_target(
    source: &CheckedProductSource,
    prepared: Result<PreparedProductTarget, Vec<Diagnostic>>,
) -> SelectedCompileOutcome {
    SelectedCompileOutcome {
        result: prepared
            .and_then(|prepared| SelectedProductTarget::select(source, prepared))
            .map_err(CompileFailure::from),
        independent_components: source.computed_build_config.independent_components(),
        tests: Vec::new(),
        test_notes: Vec::new(),
    }
}
