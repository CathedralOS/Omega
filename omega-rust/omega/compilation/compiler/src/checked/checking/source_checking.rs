// We complete one portable Product judgment (`check_product_source`, Psi 05)
// before target realization starts. Build supplies source admission, layout
// target scopes (`layout_target_scopes`), and representation choices; native
// provider selections are not checking inputs and cannot alter this checked
// program afterward.

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
    // The file that declares the selected Build machine is the Build program,
    // which the current checker judges (`lowered_route`).
    let build_program = computed_build_config
        .selected_build_machine_symbol
        .and_then(|symbol| typed.symbols.symbol_source_span(symbol))
        .and_then(|span| symbol_sources.as_deref()?.file_at(span))
        .map(|file| file.source_id);
    let layout_scopes = layout_target_scopes(&typed, build)?;
    let external_roots = build.program_roots();
    let bindings =
        provider_planning::program_entry::bound_program_entry_shapes(&typed, &build.root_bindings)?;
    // Validation subjects: the root package is checked in full; a dependency
    // only where the root package or one of the Build's selections reaches it
    // (`PackageCompilationInputs::is_validation_subject`).
    let library_slice = psi_compiler::LibrarySliceRequest {
        root_package: package_inputs.map(PackageCompilationInputs::root),
    };
    let mut demanded = external_roots.clone();
    // Binding signatures are source dependencies even when the visible
    // projection has no arguments. Retain their metadata without turning the
    // semantic schema into another executable root or importing its effects.
    demanded.extend(bindings.iter().flat_map(|binding| {
        [
            binding.semantic_schema_symbol,
            binding.requirement_symbol,
            binding.implementation,
        ]
    }));
    // The target realization table supplies canonical float operations, so,
    // as for the Build activation, the sealed catalog map and its public
    // requirements stay whether or not the program names them.
    demanded.extend(typed_trees::float_providers::catalog_realization_declarations(&typed));
    // What the Build's selections name: the providers it selected and what
    // each provides, the layout scopes' entries, and the opaque
    // representations it chose. A check claims these with their closure
    // (validation_subjects.md, "the obligations of the Build's selections").
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
    // Psi 05 re-forms each representation choice from its named conformance,
    // which may live in a dependency the program never names, so the
    // conformance is a subject alongside the opaque type it represents.
    selected.extend(
        build
            .opaque_representation_selections
            .iter()
            .flat_map(|selection| [selection.opaque(), selection.application().declaration]),
    );
    demanded.extend(selected.iter().copied());
    // A dependency's provider-default machines run for every product that
    // depends on it, and settlement finds them by their purpose rather than a
    // name, so they are preparation entries of this check.
    demanded.extend(
        typed
            .machines()
            .iter()
            .filter(|machine| {
                machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
            })
            .map(|machine| machine.symbol),
    );
    // Each accepted semantic binding names a service declaration of the
    // package it was accepted for, and selected execution rejoins it by that
    // name, so the admitted bindings are obligations of this check whether or
    // not the program calls the service.
    for binding in package_inputs
        .into_iter()
        .flat_map(PackageCompilationInputs::accepted_semantic_bindings)
    {
        demanded.extend(
            typed
                .traits()
                .iter()
                .map(|definition| definition.symbol)
                .chain(typed.machines().iter().map(|machine| machine.symbol))
                .filter(|symbol| {
                    typed.symbols.symbol_package_identity(*symbol) == Some(binding.package())
                        && typed.symbols.display_path(*symbol, "::") == binding.declaration_path()
                }),
        );
    }
    // Representation and provider requests are read back from the Build
    // machine that made them.
    demanded.extend(computed_build_config.selected_build_machine_symbol);
    // A bound root slot takes its boundary schema from the target's entry
    // contract source. Nothing in the program names that schema, so those
    // sources are subjects too.
    let mut entry_contracts = Vec::new();
    for binding in &build.root_bindings {
        if let Some((owner, _)) = binding.slot.rsplit_once("::")
            && let Ok(profile) = target::TargetProfile::from_root_slot_owner(owner)
            && let Some(package) = profile.program_entry_slot().physical_contract_package
        {
            entry_contracts.push(package);
        }
    }
    let typed = validation::execution_projection::project_subject_program(
        &typed,
        |file| {
            PackageCompilationInputs::is_validation_subject(package_inputs, file)
                || entry_contracts.iter().any(|package| {
                    file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new(package.package_relative_source()))
                })
        },
        &demanded,
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
    let subject = package_inputs.map(PackageCompilationInputs::root);
    let route_scope = super::lowered_route::RouteScope {
        entries: &external_roots,
        subject,
        selected: &selected,
        bindings: &bindings,
    };
    let mut judged = None;
    let mut route = None;
    let mut route_program = None;
    let outcome = timings.record_result(TYPED_TREES_TO_CHECKED_TREES, || {
        let started = std::time::Instant::now();
        // The checker on the lowered form judges the same trees Psi 05 is
        // given, by reference, before Psi 05 takes them; its verdict may
        // govern this compilation (`lowered_route`).
        let keep = frontier.is_some();
        let result = psi_compiler::CheckRequest {
            program: typed,
            admission: source_admission,
            previous: &previous,
            retain: reuse.retain,
            library_slice,
            // A product check judges the consumer's own package.
            checked_slices: None,
        }
        .check_inspecting_typed_frontier(&mut |trees| {
            let (judgment, published) = route_scope.judge(trees);
            route = Some(judgment);
            route_program = published;
            if keep {
                judged = Some(trees.clone());
            }
        });
        if !result.as_ref().is_ok_and(|outcome| outcome.reused) {
            artifacts::compile_timings::counters::record_product_source_check(started.elapsed());
        }
        result
    });
    let outcome = outcome.map_err(|mut diagnostics| {
        if let Some(sources) = symbol_sources.as_deref() {
            super::attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        diagnostics
    });
    let outcome = super::lowered_route::govern(
        outcome,
        route,
        &super::lowered_route::CurrentScope {
            sources: symbol_sources.as_deref(),
            build_program,
            entry_contracts: &entry_contracts,
        },
    );
    // The entries are this request's, so they are paired with the trees here,
    // before a rejection below returns and drops the Build configuration.
    if let (Some(frontier), Some(typed)) = (frontier, judged) {
        *frontier = Some(super::typed_frontier::TypedFrontier {
            typed,
            entries: external_roots.clone(),
            subject,
            selected,
            bindings,
        });
    }
    let outcome = outcome?;
    let roots = build.checked_tree_product_roots(rollback);
    let psi_compiler::SelectedCheckedSource {
        program,
        provider_defaults,
        retained,
        selection: product_selection,
    } = selections.select(outcome, roots)?;
    let mut provider_defaults = provider_defaults.to_vec();
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
    let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
    let provider_inventory = Arc::new(psi_compiler::PreparedProviderInventory::new(
        Arc::clone(&program),
        sources.exact_toolchain_sources(),
    ));
    let terminal_publication = Arc::new(
        compilation_products::ProductTerminalPublication::new(
            rollback
                .settle(&computed_build_config.config.settings.optimizations)
                .effective()
                .clone(),
            selections.portable_productions(),
        )
        .with_route(route_program),
    );
    let native_bindings = Arc::new(
        provider_planning::native_bindings::PreparedNativeBuildBindings::capture(
            provider_inventory
                .services_for(&program)
                .map_err(|error| vec![error])?,
            &computed_build_config.config.native_bindings,
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
            product: retained,
            ..Default::default()
        },
        product_selection,
    })
}

fn layout_target_scopes(
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
