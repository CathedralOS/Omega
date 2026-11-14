// Prepare the actual frozen typed owner, judge its complete structural closure,
// and retain that same owner for product consumers. No old judgment or checked
// tree projection participates in producing the application.

use artifacts::compile_timings::StageMeta;
use compilation_products::{CheckedProductSource, PreparedProductSource};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::sync::Arc;

const SOURCE_PREPARATION: StageMeta =
    StageMeta::new("typed-source-preparation", "TypedTrees", "FrozenTypedTrees");

pub(crate) fn check_product_source(
    source: PreparedProductSource,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    frontier: Option<&mut Option<super::typed_frontier::TypedFrontier>>,
) -> Result<CheckedProductSource, Vec<Diagnostic>> {
    let PreparedProductSource {
        source_completion_accounting,
        typed,
        computed_build_config,
        admission,
        build_sources,
        mut timings,
    } = source;
    build_time_evaluation::validate_constant_evaluations(&typed, &source_completion_accounting)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let build = &computed_build_config.config;
    let symbol_sources = typed.symbols.source_map();
    let layout_scopes = layout_target_scopes(&typed, build)?;
    let external_roots = build.program_roots();
    let bindings = super::entry_shapes::bound_program_entry_shapes(&typed, &build.root_bindings)?;
    // The Build's own provider selections are supplies of this compilation
    // whether or not the program reaches their slots: each chosen provider
    // is judged against the slot it names. A target package's defaults join
    // only when settlement chooses them for a requirement the accepted
    // module reaches (`PreparedProductTarget::capture`).
    let supplies = build
        .provider_selections
        .iter()
        .flat_map(crate::checked::selected_supplies)
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    for scope in &layout_scopes {
        selected.extend(scope.entry_declarations.iter().flatten().copied());
    }
    // Every admitted native supply needs its exact boundary and signature for
    // calling-policy validation and trust disclosure, even when its target or
    // requirement is unused. Claim these source declarations alongside each
    // ordinary provider selection in the structural judgment.
    selected.extend(
        build
            .native_bindings
            .iter()
            .flat_map(|supply| [supply.selection.subject, supply.selection.requirement]),
    );
    // Representation declarations are claimed source dependencies; preparation
    // explicitly refuses choices with no surviving admission route.
    selected.extend(
        build
            .opaque_representation_selections
            .iter()
            .flat_map(|selection| [selection.opaque(), selection.application().declaration]),
    );
    let source_admission = psi_compiler::SourceAdmission {
        // Roots belong to structural judgment below, rather than to a deleted
        // source-checking admission or projection.
        external_roots: &[],
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
            language_core::inline_assembly::AsmAuthorityAdmission::from_freestanding(
                build.settings.freestanding,
            )
            .with_grants(
                build.settings.privileged_services.port_io,
                build.settings.privileged_services.interrupt_table,
            ),
        ),
        wire_demands: &build.wire_compatibility_demands,
        declarations: None,
    };

    let source = timings.record_result(SOURCE_PREPARATION, || {
        psi_compiler::SourcePreparationRequest {
            program: typed,
            admission: source_admission,
        }
        .prepare()
    })?;
    let program = source.program;
    let subject = package_inputs.map(PackageCompilationInputs::root);
    let roster = compilation_products::ProductRoster {
        entries: external_roots,
        subject,
        selected,
        bindings,
        supplies,
    };
    let judged = roster.judge(Arc::clone(&program));
    if let Some(frontier) = frontier {
        *frontier = Some(super::typed_frontier::TypedFrontier {
            typed: program.as_ref().clone(),
            entries: roster.entries.clone(),
            subject,
            selected: roster.selected.clone(),
            bindings: roster.bindings.clone(),
        });
    }
    let accepted_source = judged.map_err(|mut diagnostics| {
        if let Some(sources) = symbol_sources.as_deref() {
            super::attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        diagnostics
    })?;
    let mut provider_defaults =
        build_time_evaluation::provider_declarations::retain_provider_defaults(
            build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                &program,
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
    let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
    let provider_inventory = Arc::new(
        typed_trees::provider_declarations::PreparedProviderInventory::new(
            Arc::clone(&program),
            sources.exact_toolchain_sources(),
        ),
    );
    let terminal_publication = Arc::new(
        compilation_products::ProductTerminalPublication::new(
            rollback
                .settle(&computed_build_config.config.settings.optimizations)
                .effective()
                .clone(),
        )
        .with_accepted(accepted_source, roster),
    );
    Ok(CheckedProductSource {
        source_completion_accounting,
        program,
        provider_inventory,
        terminal_publication,
        provider_defaults: provider_defaults.into(),
        computed_build_config,
        admission,
        sources,
        timings,
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
