//! Choose the code and calling plans for each requested target.
//! Selection reads saved declarations and the shared Terminal program. This
//! module then assembles the completed choices and separate source-inspection
//! access; executable admission and native emission consume the saved result.

use crate::checked::CheckedCompilation;
use crate::checked::checking::{attribute_diagnostics, timed};
use crate::checked::{PreparedProductTarget, PreparedTargetDeclarations};
use artifacts::compile_timings::{CompileTimings, EXECUTION_SETTLEMENT};
use compilation_products::{
    BuildAdmission, CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource,
    CompileFailure, RetainedCompilationChecks,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::target_machines::PreparedProductProviders;
use selected_dispatch::{ProviderBindingRequest, SelectedExecution, prepare_selected_execution};
use std::sync::Arc;

/// One refusal as the diagnostic list every step in this module returns: each
/// provider-inventory query answers once, so its single refusal is the whole
/// list.
fn one_refusal<T>(refused: Result<T, Diagnostic>) -> Result<T, Vec<Diagnostic>> {
    refused.map_err(|error| vec![error])
}

/// One target's completed choices beside shared code and explicit inspection access.
/// Selection and native declaration preparation are complete; no later
/// installation phase reopens source or fills in this result. Source-only
/// consumers request their checked-instance projection at their own boundary.
pub(crate) struct SelectedProviderCustody {
    pub(crate) terminal_publication: Arc<compilation_products::ProductTerminalPublication>,
    pub(crate) source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    pub(crate) execution: SelectedExecution,
    pub(crate) source_inspection: Arc<provider_planning::SelectedProviderSource>,
    pub(crate) settlement: provider_planning::ProviderSettlement,
    pub(crate) build: Arc<compilation_products::ComputedBuildConfig>,
    pub(crate) build_admission: BuildAdmission,
    pub(crate) provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub(crate) product_selection: Option<psi_compiler::CheckedTreeProductSelection>,
    pub(crate) sources: Arc<package_compilation::CheckedSourceCustody>,
    pub(crate) timings: CompileTimings,
}

/// A checked source application ready for Terminal production. Review stops
/// validate task demands directly; executable products use their artifact's
/// demands in native proposal construction instead.
pub(crate) struct SelectedProductTarget {
    pub(crate) runtime_policy: Arc<package_evidence::permission_acceptance::PreparedRuntimePolicy>,
    pub(crate) custody: SelectedProviderCustody,
    pub(crate) retained_checks: RetainedCompilationChecks,
    pub(crate) independent_components:
        Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
}

impl SelectedProductTarget {
    pub(crate) fn select(
        source: &CheckedProductSource,
        prepared: PreparedProductTarget,
    ) -> Result<Self, Vec<Diagnostic>> {
        let runtime_policy = Arc::clone(&prepared.runtime_policy);
        let custody = select_product_providers(source, prepared)?;
        // Direct source consumers do not attach a package session's products.
        // Unprepared Independent demands must not look like a complete empty set.
        let mut independent_components = Ok(Vec::new());
        for selected in &custody.execution.selected_provider_provenance {
            if selected
                .selected_by
                .composition_mode()
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                == provider_planning::CompositionMode::Independent
            {
                independent_components = Err(
                    "independent native production requires exactly one complete retained producing product".to_owned(),
                );
            }
        }
        Ok(Self {
            runtime_policy,
            custody,
            retained_checks: source.retained_checks.clone(),
            independent_components,
        })
    }

    /// Attach only products whose whole descriptive publication and realized
    /// target answer this selection. Different native proposals under the same
    /// description remain competing inputs, never a first-match cache hit.
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
                // Descriptive checking may have no executable product. Actual
                // production must separately require the complete product set.
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
            // Descriptive stops need not have an executable product. Retain a
            // native-only refusal, but still check every available producing
            // join now: a bad ABI is not hidden by another missing product.
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
        Ok(self.into_checked_compilation())
    }

    pub(crate) fn check_declaration_requirements(
        &self,
        terminal: Option<&terminal_psi::TerminalModule>,
    ) -> Result<(), Vec<Diagnostic>> {
        // Selection already published Terminal, even when no proof condition
        // needed evaluation. Read its task demands instead of lowering source again.
        let execution = &self.custody.execution;
        let module = terminal.unwrap_or_else(|| {
            self.custody
                .settlement
                .calling_program
                .artifact
                .semantic_module()
        });
        check_terminal_task_activations(
            module,
            execution.plans(),
            execution.selected_target_profile,
        )
    }

    /// Assemble the already-prepared result. Executable products discharge task
    /// demands against Terminal in native proposal construction, not here.
    pub(crate) fn into_checked_compilation(self) -> CheckedCompilation {
        checked_compilation(
            self.custody,
            self.retained_checks,
            self.independent_components,
        )
    }
}

/// Publication retains all declaration task obligations, not just executable
/// reach. Check them against the same canonical identities used by native
/// admission instead of reopening and lowering the checked source again.
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

/// Discovery survives selection failure so package review can acquire the
/// missing independent component without treating failure as authority.
pub(crate) struct SelectedCompileOutcome {
    pub(crate) result: Result<SelectedProductTarget, CompileFailure>,
    pub(crate) independent_components: compilation_products::IndependentComponentDiscovery,
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

fn select_checked_providers(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    declarations: Result<PreparedTargetDeclarations, Vec<Diagnostic>>,
    deployment: Result<(), Vec<Diagnostic>>,
    binding_declarations: &selected_dispatch::PreparedBindingDeclarations,
    direct_calls: Result<
        std::sync::Arc<
            language_semantics::direct_calls::DirectCallDeclarations<symbols::SymbolHandle>,
        >,
        Diagnostic,
    >,
    timings: &mut CompileTimings,
) -> Result<(SelectedExecution, provider_planning::ProviderSettlement), Vec<Diagnostic>> {
    let build_config = &source.computed_build_config.config;
    let x86_scalar_fma_provider = build_config
        .settings
        .x86_deployment_claim
        .admit_for(selected_target_profile)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let PreparedTargetDeclarations {
        entry,
        providers: prepared_providers,
        entry_service_requirements,
    } = declarations?;
    let provider_planning::CheckedProviderSelection {
        settlement: providers,
        selected_provider_plan_facts,
        selected_provider_provenance,
        selected_declarations,
        native_provider_sources,
    } = prepare_target_provider_selection(
        source,
        selected_target_profile,
        package_inputs,
        prepared_providers,
        deployment,
        timings,
    )?;
    let mut selected_program_entry = entry?
        .map(|entry| entry.settle_calling_plans(&providers.boundary_calling_plan_realizations))
        .transpose()?;
    if let Some(entry) = &mut selected_program_entry {
        entry.bind_physical_adapter(
            providers.calling_program.artifact.semantic_module(),
            &selected_provider_provenance,
            &providers.provider_declarations,
        )?;
    }
    provider_planning::target_machines::reject_reached_inert_sibling_calls(
        one_refusal(direct_calls)?.as_ref(),
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
        selected_program_entry
            .as_ref()
            .map(|entry| entry.machine_symbol()),
        &selected_provider_provenance,
    )?;
    let root_grants = build_config.root_grant_selectors();
    let providers = providers.finish_row_declarations()?;
    let execution = prepare_selected_execution(ProviderBindingRequest {
        declarations: binding_declarations,
        selected_declarations,
        native_provider_sources,
        entry_service_requirements,
        package_inputs,
        boundary_calling_plan_realizations: &providers.boundary_calling_plan_realizations,
        provider_candidates: &providers.provider_candidates,
        boundary_provider_demands: &providers.boundary_provider_demands,
        provider_declarations: &providers.provider_declarations,
        calling_program: &providers.calling_program,
        exact_toolchain_sources: source.sources.exact_toolchain_sources(),
        selected_provider_plan_facts,
        selected_provider_provenance,
        root_grants: &root_grants,
        authored_root_grants: &build_config.grants,
        provider_planning_target_profile: providers.planning_target_profile,
        selected_target_profile,
        selected_program_entry,
        x86_scalar_fma_provider,
    })?;
    Ok((execution, providers))
}

/// Prepare the exact provider and ABI inputs for either a product or an
/// authored-target proof application. Neither use bypasses provider admission;
/// only executable products need a ProgramEntry and deployment claim.
fn prepare_target_provider_selection(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    prepared_providers: Result<PreparedProductProviders, Vec<Diagnostic>>,
    deployment: Result<(), Vec<Diagnostic>>,
    timings: &mut CompileTimings,
) -> Result<provider_planning::CheckedProviderSelection, Vec<Diagnostic>> {
    let build_config = &source.computed_build_config.config;
    let code = crate::terminal::retain_calling_program(source, timings)?;
    let calling = provider_planning::calling_policy_plans::prepare_boundary_calling_plans(
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
        package_inputs,
        &code,
    )?;
    let boundary_calling_plan_realizations = calling.evaluate(&code)?;
    deployment?;
    provider_planning::settle_checked_providers(
        selected_target_profile,
        package_inputs,
        prepared_providers,
        &build_config.accepted_component_assumption_digests(),
        boundary_calling_plan_realizations,
        code,
    )
}

/// Select this target from saved declarations and shared code, then assemble
/// source-inspection access for review. Execution admission follows separately.
fn select_product_providers(
    source: &CheckedProductSource,
    prepared: PreparedProductTarget,
) -> Result<SelectedProviderCustody, Vec<Diagnostic>> {
    let PreparedProductTarget {
        runtime_policy: _,
        profile: selected_target_profile,
        package_inputs,
        declarations,
        deployment,
        timings: preparation_timings,
        binding_declarations,
        direct_calls,
    } = prepared;
    let package_inputs = package_inputs.as_ref();
    let mut timings = source.timings.clone();
    timings.extend(&preparation_timings);
    let selection = timed(&mut timings, EXECUTION_SETTLEMENT, |timings| {
        select_checked_providers(
            source,
            selected_target_profile,
            package_inputs,
            declarations,
            deployment,
            &binding_declarations,
            direct_calls,
            timings,
        )
    });
    let selection = selection.map_err(|mut diagnostics| {
        if let Some(sources) = source.program.typed.symbols.source_map().as_deref() {
            attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        diagnostics
    })?;
    let (execution, settlement) = selection;
    let source_inspection = Arc::new(provider_planning::SelectedProviderSource::new(
        Arc::clone(&source.declarations),
        Arc::clone(&source.provider_inventory),
        Arc::clone(&execution.selected_provider_plan_facts),
    ));
    Ok(SelectedProviderCustody {
        source_inspection,
        terminal_publication: Arc::clone(&source.terminal_publication),
        source_completion_accounting: source.source_completion_accounting.clone(),
        execution,
        settlement,
        build: Arc::clone(&source.computed_build_config),
        build_admission: source.admission.clone().for_target(selected_target_profile),
        provider_defaults: Arc::clone(&source.provider_defaults),
        product_selection: source.product_selection.clone(),
        sources: Arc::clone(&source.sources),
        timings,
    })
}

/// Assemble source custody and its completed selected execution. This is not
/// artifact installation: no source query, selection or fallible phase remains.
fn checked_compilation(
    selection: SelectedProviderCustody,
    retained_checks: RetainedCompilationChecks,
    independent_components: Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
) -> CheckedCompilation {
    let SelectedProviderCustody {
        terminal_publication,
        source_completion_accounting,
        execution,
        source_inspection,
        settlement,
        build,
        build_admission,
        provider_defaults,
        product_selection,
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
            product_selection,
        },
        sources,
        timings,
        terminal_publication,
    )
    .with_source_completion_accounting(source_completion_accounting)
    .with_settled_checks(retained_checks)
}

/// Select one target against the shared source judgment. The consumer chooses
/// its actual stop; source checking never installs an executable as a side effect.
pub(crate) fn select_product_target(
    source: &CheckedProductSource,
    prepared: Result<PreparedProductTarget, Vec<Diagnostic>>,
) -> SelectedCompileOutcome {
    SelectedCompileOutcome {
        result: prepared
            .and_then(|prepared| SelectedProductTarget::select(source, prepared))
            .map_err(CompileFailure::from),
        independent_components: source.computed_build_config.independent_components(),
    }
}
