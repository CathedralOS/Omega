//! Complete one portable Product judgment before target realization starts.
//!
//! Build supplies source admission and representation choices. Native provider
//! selections are not checking inputs and cannot change this program afterward.

use artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES;
use compilation_products::{CheckedProductSource, PreparedProductSource, SettledCheckReuse};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::sync::Arc;

pub(crate) fn check_product_source(
    source: PreparedProductSource,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    reuse: SettledCheckReuse,
    selections: &mut psi_compiler::ProductSelections,
) -> Result<CheckedProductSource, Vec<Diagnostic>> {
    let PreparedProductSource {
        source_completion_accounting,
        typed,
        computed_build_config,
        admission,
        build_sources,
        mut timings,
        retained_build_check,
    } = source;
    source_completion_accounting
        .validate_charges()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let build = &computed_build_config.config;
    let symbol_sources = typed.symbols.source_map();
    let layout_scopes = layout_target_scopes(&typed, build)?;
    let source_admission = psi_compiler::SourceAdmission {
        layout_scopes: &layout_scopes,
        layout_selection_authority: package_inputs.map(|inputs| {
            Arc::new(inputs.clone()) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
        completion: match package_inputs.and_then(|inputs| {
            inputs.accepted_semantic_binding(
                package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            )
        }) {
            Some(binding) => {
                validation::source_completion::SourceCompletionAdmission::AcceptedProcessExit {
                    package: binding.package(),
                    declaration_path: binding.declaration_path(),
                }
            }
            None if package_inputs
                .is_some_and(PackageCompilationInputs::semantic_role_discovery) =>
            {
                validation::source_completion::SourceCompletionAdmission::DiscoverProcessExit
            }
            None => validation::source_completion::SourceCompletionAdmission::None,
        },
        representations: psi_compiler::ProductRepresentationInput {
            activation: computed_build_config.selected_build_machine_symbol,
            declared: &computed_build_config.declared_product_selections,
            executed: &computed_build_config.product_selection_requests,
            retained: &build.opaque_representation_selections,
        },
        asm_authority: Some(
            validation::AsmAuthorityAdmission::from_freestanding(build.settings.freestanding)
                .with_grants(
                    build.settings.privileged_services.port_io,
                    build.settings.privileged_services.interrupt_table,
                ),
        ),
        wire_demands: &build.wire_compatibility_demands,
        declarations: package_inputs
            .map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
    };
    let previous: Vec<_> = reuse
        .prior
        .product
        .iter()
        .chain(reuse.prior.sibling_product_checks.iter())
        .collect();
    let outcome = timings
        .record_result(TYPED_TREES_TO_CHECKED_TREES, || {
            let started = std::time::Instant::now();
            let result = psi_compiler::CheckRequest {
                program: typed,
                admission: source_admission,
                previous: &previous,
                retain: reuse.retain,
            }
            .check();
            if !result.as_ref().is_ok_and(|outcome| outcome.reused) {
                artifacts::compile_timings::counters::record_settled_check(started.elapsed());
            }
            result
        })
        .map_err(|mut diagnostics| {
            if let Some(sources) = symbol_sources.as_deref() {
                super::attribute_diagnostics(&mut diagnostics, sources, package_inputs);
            }
            diagnostics
        })?;
    // Product pruning is a Psi operation over all declared roots, not a
    // per-target copy of the checked program. Native reachability chooses the
    // selected entry later, without changing this portable judgment.
    let roots = build.checked_tree_product_roots(rollback);
    let psi_compiler::SelectedCheckedSource {
        program,
        provider_defaults,
        retained,
        selection: product_selection,
    } = selections.select(outcome, roots)?;
    // One default inventory feeds initial settlement, Terminal demand roots
    // and every target proposal. Package handoffs have the same default tier
    // on all three routes; only the root Build supplies overrides.
    let mut provider_defaults = provider_defaults.to_vec();
    for selection in &build.package_provider_selections {
        provider_defaults.push(
            build_time_evaluation::provider_declarations::TargetProviderDefaults {
                target: selection
                    .target
                    .map(|target| target.canonical_name().to_owned()),
                machines: Vec::new(),
                selections: vec![selection.clone()],
            },
        );
    }
    let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
    let accepted_template_classifications =
        trust_model::AcceptedTemplateClassifications::capture(&program);
    let provider_inventory = Arc::new(psi_compiler::PreparedProviderInventory::new(Arc::clone(
        &program,
    )));
    Ok(CheckedProductSource {
        source_completion_accounting,
        program,
        provider_inventory,
        external_declarations: Arc::default(),
        provider_defaults: provider_defaults.into(),
        computed_build_config,
        admission,
        sources,
        timings,
        accepted_template_classifications,
        retained_checks: compilation_products::RetainedCompilationChecks {
            build: retained_build_check,
            product: retained,
            ..Default::default()
        },
        product_selection,
    })
}

/// Source geometry checks every declared body under its explicit target scope.
/// Build contributes checking profiles, without pruning implementation bodies;
/// reusable source receives the explicit target catalog, never the build host.
fn layout_target_scopes(
    typed: &typed_trees::TypedTrees,
    build: &compilation_products::BuildConfig,
) -> Result<Vec<build_time_evaluation::FixedLayoutTargetScope>, Vec<Diagnostic>> {
    if !typed_trees::references::collect_program_type_occurrences(typed)
        .into_iter()
        .any(|occurrence| {
            typed
                .layout_storage_arguments(occurrence.reference)
                .is_some()
        })
    {
        return Ok(Vec::new());
    }
    let choices = build
        .provider_selections
        .iter()
        .chain(&build.package_provider_selections)
        .collect::<Vec<_>>();
    let mut profiles = Vec::new();
    for binding in &build.root_bindings {
        let (owner, _) = binding.slot.rsplit_once("::").ok_or_else(|| {
            vec![Diagnostic::error(
                "fixed representation scope has an invalid Build root slot",
            )]
        })?;
        let profile =
            target::TargetProfile::from_root_slot_owner(owner).map_err(|error| vec![error])?;
        if !profiles.contains(&profile) {
            profiles.push(profile);
        }
    }
    for choice in &choices {
        if let Some(target) = choice.target {
            let profile =
                target::TargetProfile::from_canonical_target_name(target.canonical_name())
                    .map_err(|error| vec![error])?;
            if !profiles.contains(&profile) {
                profiles.push(profile);
            }
        }
    }
    let reusable = profiles.is_empty();
    if reusable {
        profiles.extend(
            target::TargetProfile::ALL
                .into_iter()
                .filter(|profile| profile.layout_target_facts().is_some()),
        );
    }
    // A body is checked even when no Build root or selected provider invokes
    // it. Preserve its authored target context independently of deployment.
    for machine in typed.machines() {
        for target in [
            typed.machine_authored_target(machine),
            typed.source_target_context(machine),
        ]
        .into_iter()
        .flatten()
        {
            let profile = target::TargetProfile::from_canonical_target_name(target)
                .map_err(|error| vec![error])?;
            if !profiles.contains(&profile) {
                profiles.push(profile);
            }
        }
    }
    let mut scopes = Vec::new();
    for profile in profiles {
        let mut entries = Vec::new();
        for binding in &build.root_bindings {
            let (owner, _) = binding
                .slot
                .rsplit_once("::")
                .expect("root slots were validated above");
            let declared =
                target::TargetProfile::from_root_slot_owner(owner).map_err(|error| vec![error])?;
            if declared == profile && !entries.contains(&binding.implementation_symbol) {
                entries.push(binding.implementation_symbol);
            }
        }
        // Every supplied declaration is a checking candidate. Root/package
        // precedence and eventual invocation belong to provider settlement.
        let providers = choices
            .iter()
            .filter(|choice| {
                choice
                    .target
                    .is_none_or(|target| target.canonical_name() == profile.target_name())
            })
            .map(|choice| (*choice).clone())
            .collect();
        scopes.push(build_time_evaluation::FixedLayoutTargetScope {
            target: profile.target_name().to_owned(),
            facts: profile.layout_target_facts(),
            entry_declarations: (!reusable).then_some(entries),
            provider_declarations: providers,
        });
    }
    Ok(scopes)
}
