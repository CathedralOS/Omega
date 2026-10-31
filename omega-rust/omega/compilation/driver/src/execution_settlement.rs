//! Prepare each target's complete selected execution against immutable checked
//! source and Build rows. Terminal production borrows that result; converting
//! it to a checked compilation assembles custody, not another compiler phase.
//! Source-dependent selection remains migration work. Actual artifact demand,
//! receiving permissions and native realization belong to the later native join.

use crate::checked::CheckedCompilation;
use crate::checked::checking::{attribute_diagnostics, timed};
use artifacts::compile_timings::{CompileTimings, EXECUTION_SETTLEMENT};
use compilation_products::{
    BuildAdmission, CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource,
    CompileFailure, RetainedCompilationChecks,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use selected_dispatch::{ProviderBindingRequest, SelectedExecution, prepare_selected_execution};
use std::sync::Arc;

/// One target's complete provider selection beside the shared source custody.
/// Terminal production borrows this record to mint the target-neutral artifact.
/// Selection and native declaration preparation are complete; no later
/// installation phase reopens source or fills in this result. Source-only
/// consumers request their checked-instance projection at their own boundary.
pub(crate) struct SelectedProviderCustody {
    pub(crate) source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    pub(crate) execution: SelectedExecution,
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
    pub(crate) custody: SelectedProviderCustody,
    pub(crate) retained_checks: RetainedCompilationChecks,
    pub(crate) independent_components:
        Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
}

impl SelectedProductTarget {
    pub(crate) fn select(
        source: &CheckedProductSource,
        target: Option<target::TargetProfile>,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut package_inputs = package_inputs.cloned();
        if let Some(inputs) = package_inputs.as_ref()
            && let Some(binding) =
                provider_planning::program_entry::discover_explicit_entry_binding(
                    source
                        .provider_inventory
                        .services_for(&source.program)
                        .map_err(|error| vec![error])?,
                    &source.computed_build_config.config.root_bindings,
                    target,
                    &source.computed_build_config.config.provider_selections,
                    inputs,
                )?
        {
            let mut bindings = inputs
                .accepted_semantic_bindings()
                .cloned()
                .collect::<Vec<_>>();
            bindings.push(binding);
            package_inputs = Some(
                inputs
                    .clone()
                    .with_accepted_semantic_bindings(bindings)
                    .map_err(|errors| {
                        errors
                            .into_iter()
                            .map(|error| {
                                Diagnostic::error(format!("entry candidate inputs: {error:?}"))
                            })
                            .collect::<Vec<_>>()
                    })?,
            );
        }
        let custody = select_product_providers(source, target, package_inputs.as_ref())?;
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
        let facts = execution.provider_source.plans();
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
            let selected_inputs = (|| {
                let mut components = verified
                    .iter()
                    .filter(|component| component.realizes_selected_plan(&selected.plan).is_ok());
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
                Ok((component, product, plan))
            })();
            let (component, product, plan) = match selected_inputs {
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
        // A source-only stop owes selected-instance coverage without native
        // realization. Executable products obtain it from Terminal instead.
        let execution = &self.custody.execution;
        execution.provider_source.project_instances()?;
        match terminal {
            Some(module) => check_terminal_task_activations(
                module,
                execution.provider_source.plans(),
                execution.selected_target_profile,
            ),
            None => {
                // A declaration-only stop may deliberately avoid publication.
                // Project only there; every published product already carries
                // this complete source obligation bundle, including inert uses.
                let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
                let demands = psi_compiler::lower_task_activation_demands(
                    execution.provider_source.program(),
                    &mut identities,
                )?;
                task_plans::realize_task_activations(
                    &demands,
                    &identities.build(),
                    execution.provider_source.plans(),
                    execution
                        .selected_target_profile
                        .unwrap_or_else(target::TargetProfile::host),
                )?;
                Ok(())
            }
        }
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
) -> Result<(SelectedExecution, provider_planning::ProviderSettlement), Vec<Diagnostic>> {
    let typed = &source.program.typed;
    let build_config = &source.computed_build_config.config;
    let x86_scalar_fma_provider = build_config
        .settings
        .x86_deployment_claim
        .admit_for(selected_target_profile)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let provider_planning::CheckedProviderSelection {
        settlement: providers,
        selected_provider_plan_facts,
        selected_provider_provenance,
    } = prepare_target_provider_selection(source, selected_target_profile, package_inputs)?;
    let mut selected_program_entry =
        provider_planning::program_entry::select_compiler_program_entry(
            source
                .provider_inventory
                .services_for(&source.program)
                .map_err(|error| vec![error])?,
            &build_config.root_bindings,
            selected_target_profile,
            &providers.boundary_calling_plan_realizations,
            package_inputs,
        )?;
    if let Some(entry) = &mut selected_program_entry {
        entry.bind_physical_adapter(typed, &selected_provider_provenance)?;
    }
    provider_planning::target_machines::reject_reached_inert_sibling_calls(
        source
            .provider_inventory
            .direct_calls_for(&source.program)
            .map_err(|error| vec![error])?,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
        selected_program_entry
            .as_ref()
            .map(|entry| entry.machine_symbol()),
        &selected_provider_provenance,
    )?;
    let root_grants = build_config.root_grant_selectors();
    let providers =
        providers.prepare_row_declarations(typed, source.sources.exact_toolchain_sources())?;
    let execution = prepare_selected_execution(
        Arc::clone(&source.program),
        source.accepted_template_classifications.clone(),
        ProviderBindingRequest {
            provider_inventory: &source.provider_inventory,
            external_declarations: source.external_declarations.as_ref(),
            exact_toolchain_sources: source.sources.exact_toolchain_sources(),
            package_inputs,
            boundary_calling_plan_realizations: &providers.boundary_calling_plan_realizations,
            provider_candidates: &providers.provider_candidates,
            selected_provider_plan_facts,
            selected_provider_provenance,
            root_grants: &root_grants,
            authored_root_grants: &build_config.grants,
            provider_planning_target_profile: providers.planning_target_profile,
            selected_target_profile,
            selected_program_entry,
            x86_scalar_fma_provider,
        },
    )?;
    Ok((execution, providers))
}

/// Prepare the exact provider and ABI inputs for either a product or an
/// authored-target proof application. Neither use bypasses provider admission;
/// only executable products need a ProgramEntry and deployment claim.
fn prepare_target_provider_selection(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<provider_planning::CheckedProviderSelection, Vec<Diagnostic>> {
    let typed = &source.program.typed;
    let build_config = &source.computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let mut boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            &source.program,
            selected_target_profile.unwrap_or_else(target::TargetProfile::host),
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    // Provider schemas and provenance commit to the complete calling
    // application. Close target callback geometry before either captures it;
    // otherwise nominal placements retain no private materialization and a
    // later closure would invalidate the already-captured plan commitment.
    // This consumes the shared source judgment, never another Psi check.
    if selected_target_profile.is_some() {
        provider_planning::calling_policy_plans::close_outbound_callback_materializations(
            typed,
            &mut boundary_calling_plan_realizations,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    }
    let settled_target_machines = provider_planning::target_machines::settle_provider_defaults(
        source
            .provider_inventory
            .origins_for(&source.program)
            .map_err(|error| vec![error])?,
        &source.provider_defaults,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
    );
    provider_planning::settle_checked_providers(
        &source.program,
        source
            .provider_inventory
            .services_for(&source.program)
            .map_err(|error| vec![error])?,
        source
            .provider_inventory
            .uses_for(&source.program)
            .map_err(|error| vec![error])?,
        source
            .provider_inventory
            .direct_calls_for(&source.program)
            .map_err(|error| vec![error])?,
        settled_target_machines,
        selected_target_profile,
        package_inputs,
        &build_config.provider_selections,
        &build_config.native_bindings,
        &build_config.assumed_bindings,
        &build_config.accepted_component_assumption_digests(),
        boundary_calling_plan_realizations,
        &build_config.opaque_representation_selections,
        &source.program.facts.synchronous_invocations,
    )
}

/// Prepare this target's complete source-side custody without producing an
/// artifact or admitting its native execution.
pub(crate) fn select_product_providers(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<SelectedProviderCustody, Vec<Diagnostic>> {
    let mut timings = source.timings.clone();
    let selection = timed(&mut timings, EXECUTION_SETTLEMENT, |_| {
        select_checked_providers(source, selected_target_profile, package_inputs)
    });
    let selection = selection.map_err(|mut diagnostics| {
        if let Some(sources) = source.program.typed.symbols.source_map().as_deref() {
            attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        diagnostics
    })?;
    let (execution, settlement) = selection;
    Ok(SelectedProviderCustody {
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
        source_completion_accounting,
        execution,
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
            build,
            build_admission,
            provider_defaults,
            providers: settlement,
            independent_components,
            product_selection,
        },
        sources,
        timings,
    )
    .with_source_completion_accounting(source_completion_accounting)
    .with_settled_checks(retained_checks)
}

/// Select one target against the shared source judgment. The consumer chooses
/// its actual stop; source checking never installs an executable as a side effect.
pub(crate) fn select_product_target(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> SelectedCompileOutcome {
    SelectedCompileOutcome {
        result: SelectedProductTarget::select(source, selected_target_profile, package_inputs)
            .map_err(CompileFailure::from),
        independent_components: source.computed_build_config.independent_components(),
    }
}
