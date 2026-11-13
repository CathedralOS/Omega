// Prepare one immutable source frontier, judge it with the lowered-form
// checker, and retain exactly the artifact and declaration packet it publishes.
// Native selection consumes that packet; no retired body verdict or lowering
// output can substitute for it.

use compilation_products::{CheckedProductSource, PreparedProductSource, SettledCheckReuse};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::sync::Arc;

pub(crate) fn check_product_source(
    source: PreparedProductSource,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    _reuse: SettledCheckReuse,
    selections: &mut psi_compiler::ProductSelections,
    frontier: Option<&mut Option<super::typed_frontier::TypedFrontier>>,
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
    let mut external_roots = build.program_roots();
    let bindings =
        provider_planning::program_entry::bound_program_entry_shapes(&typed, &build.root_bindings)?;
    let mut selected = Vec::new();
    for selection in layout_scopes
        .iter()
        .flat_map(|scope| &scope.provider_declarations)
        .chain(&build.provider_selections)
        .chain(&build.default_provider_selections)
    {
        use build_time_evaluation::provider_declarations::ProviderSelectionSubject;
        selected.push(selection.provider_type.symbol);
        match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                selected.push(identity.symbol);
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                selected.extend(family.coordinates().iter().map(|row| row.symbol));
            }
        }
    }
    for scope in &layout_scopes {
        selected.extend(scope.entry_declarations.iter().flatten().copied());
    }
    // Every admitted native supply needs its exact boundary and signature for
    // calling-policy validation and trust disclosure, even when its target or
    // requirement is unused. Keep that checked closure through subject
    // projection just as for an ordinary provider selection.
    selected.extend(
        build
            .native_bindings
            .iter()
            .flat_map(|supply| [supply.selection.subject, supply.selection.requirement]),
    );
    // Representation choices name both an opaque type and its conformance.
    // Keep both as checking subjects even when the conformance lives in an
    // otherwise unused dependency.
    selected.extend(
        build
            .opaque_representation_selections
            .iter()
            .flat_map(|selection| [selection.opaque(), selection.application().declaration]),
    );
    let source_admission = psi_compiler::SourceAdmission {
        external_roots: &external_roots,
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
                typed_trees::boundary_completions::SourceCompletionAdmission::AcceptedProcessExit {
                    package: binding.package(),
                    declaration_path: binding.declaration_path(),
                }
            }
            None if package_inputs
                .is_some_and(PackageCompilationInputs::semantic_role_discovery) =>
            {
                typed_trees::boundary_completions::SourceCompletionAdmission::DiscoverProcessExit
            }
            None => typed_trees::boundary_completions::SourceCompletionAdmission::None,
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
    let typed = Arc::new(psi_compiler::prepare_product_typed_frontier(
        typed,
        &source_admission,
    )?);
    let mut provider_defaults =
        build_time_evaluation::provider_declarations::retain_provider_defaults(
            build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ),
        )?;
    for selection in &build.default_provider_selections {
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
    let code_roots =
        crate::terminal::source_roots::collect(&typed, &computed_build_config, &provider_defaults)?;
    // Keep the Build's first actual entry as the executable entry. Additional
    // source-code obligations are checked roots, never extra invocation edges.
    if !external_roots.is_empty() {
        for machine in code_roots.machines {
            if !external_roots.contains(&machine) {
                external_roots.push(machine);
            }
        }
    }
    selected.extend(code_roots.boundaries);
    selected.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    selected.dedup();
    let subject = package_inputs.map(PackageCompilationInputs::root);
    let route_scope = super::lowered_route::RouteScope {
        entries: &external_roots,
        subject,
        selected: &selected,
        bindings: &bindings,
    };
    // Keeping a frontier is an explicit inspection request. The normal
    // product path shares one Arc through checking, metadata and emission.
    if let Some(frontier) = frontier {
        *frontier = Some(super::typed_frontier::TypedFrontier {
            typed: typed.as_ref().clone(),
            entries: external_roots.clone(),
            subject,
            selected: selected.clone(),
            bindings: bindings.clone(),
        });
    }
    let started = std::time::Instant::now();
    let judged = timings.record_result(
        artifacts::compile_timings::StageMeta::new("Psi check", "TypedTrees", "AcceptedPsi"),
        || {
            let (verdict, published) = route_scope.judge(&typed);
            verdict.verdict()?;
            published
        },
    );
    artifacts::compile_timings::counters::record_product_source_check(started.elapsed());
    let program = judged.map_err(|mut diagnostics| {
        if let Some(sources) = symbol_sources.as_deref() {
            super::attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        diagnostics
    })?;
    if !Arc::ptr_eq(&typed, program.typed()) {
        return Err(vec![Diagnostic::error(
            "checked product publication changed its source frontier",
        )]);
    }
    let sources = Arc::new(build_sources.seal(&typed, package_inputs)?);
    let provider_inventory = Arc::new(provider_planning::PreparedProviderInventory::new(
        Arc::clone(&program),
        sources.exact_toolchain_sources(),
    ));
    let terminal_publication = Arc::new(
        compilation_products::ProductTerminalPublication::new(
            rollback
                .settle(&build.settings.optimizations)
                .effective()
                .clone(),
            selections.portable_productions(),
        )
        .with_checked_application(Ok(Arc::clone(&program))),
    );
    let native_bindings = Arc::new(
        provider_planning::native_bindings::PreparedNativeBuildBindings::capture(
            provider_inventory
                .services_for(&program)
                .map_err(|error| vec![error])?,
            &build.native_bindings,
        ),
    );
    Ok(CheckedProductSource {
        source_completion_accounting,
        program,
        provider_inventory,
        native_bindings,
        terminal_publication,
        calling_program: Arc::default(),
        binding_declarations: Arc::default(),
        provider_defaults: provider_defaults.into(),
        computed_build_config,
        admission,
        sources,
        timings,
        retained_checks: compilation_products::RetainedCompilationChecks {
            build: retained_build_check,
            ..Default::default()
        },
        // The checked module owns its closure directly. There is no retired
        // checked-tree pruning operation or receipt on this source path.
        product_selection: None,
    })
}

pub(super) fn layout_target_scopes(
    typed: &typed_trees::TypedTrees,
    build: &compilation_products::BuildConfig,
) -> Result<Vec<build_time_evaluation::FixedLayoutTargetScope>, Vec<Diagnostic>> {
    let choices = build
        .provider_selections
        .iter()
        .chain(&build.default_provider_selections)
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
