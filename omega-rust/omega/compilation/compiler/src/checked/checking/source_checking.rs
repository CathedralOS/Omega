// Prepare the actual frozen typed owner, judge its complete structural closure,
// and retain that same owner for product consumers. No old judgment or checked
// tree projection participates in producing the application.

use artifacts::compile_timings::StageMeta;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use products::{CheckedProductSource, CompileFailure, PreparedProductSource};
use std::sync::Arc;

const SOURCE_PREPARATION: StageMeta =
    StageMeta::new("typed-source-preparation", "TypedTrees", "FrozenTypedTrees");

pub(crate) fn check_product_source(
    source: PreparedProductSource,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    inspection: Option<&str>,
    frontier: Option<&mut Option<super::typed_frontier::TypedFrontier>>,
) -> Result<CheckedProductSource, CompileFailure> {
    let build_accounting = source.build_accounting.clone();
    let mut accounting = source.source_completion_accounting.clone();
    let result = (|| -> Result<CheckedProductSource, Vec<Diagnostic>> {
        let PreparedProductSource {
            build_accounting: _,
            source_completion_accounting: _,
            typed,
            computed_build_config,
            admission,
            build_sources,
            mut timings,
        } = source;
        accounting
            .validate_charges()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let build = &computed_build_config.config;
        let symbol_sources = typed.symbols.source_map();
        // A dependency's provider defaults are laid out only for the targets
        // that choose them, and targets choose from the judged module. The
        // first pass lays out the entries and the Build's own selections. When
        // a target then chooses a default the source was not laid out for, the
        // source is prepared again from this unprepared copy, which exists
        // only when the Build has dependency defaults.
        let unprepared = (!build.default_provider_selections.is_empty()).then(|| typed.clone());
        let mut pass = ProductSourcePass {
            computed_build_config: &computed_build_config,
            package_inputs,
            rollback,
            inspection,
            frontier,
            symbol_sources,
            build_accounting: &build_accounting,
            initial_accounting: accounting.clone(),
            accounting: &mut accounting,
            timings: &mut timings,
        };
        let mut judged = prepare_and_judge(&mut pass, typed, &[])?;
        let terminal_publication = loop {
            match judged.judgment {
                crate::checked::SupplyJudgment::Supplied(publication) => break publication,
                // The chosen set only grows, bounded by the dependency
                // defaults on each target, so this loop ends.
                crate::checked::SupplyJudgment::PrepareLayouts(chosen) => {
                    let typed = unprepared.clone().ok_or_else(|| {
                        vec![Diagnostic::error(
                            "a target chose a provider default the Build does not declare",
                        )]
                    })?;
                    judged = prepare_and_judge(&mut pass, typed, &chosen)?;
                }
            }
        };
        drop(pass);
        let JudgedSource {
            program,
            source_completion_accounting,
            provider_defaults,
            ..
        } = judged;
        let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
        let provider_inventory = Arc::new(
            typed_trees::provider_declarations::PreparedProviderInventory::new(
                Arc::clone(&program),
                sources.exact_toolchain_sources(),
            ),
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
    })();
    let mut spent = build_accounting;
    spent.append(accounting);
    result.map_err(|diagnostics| CompileFailure::source_completion(diagnostics, spent))
}

/// What every preparation of one Product source shares.
struct ProductSourcePass<'a, 'f> {
    computed_build_config: &'a products::ComputedBuildConfig,
    package_inputs: Option<&'a PackageCompilationInputs>,
    rollback: &'a crate::OptimizationRollback,
    /// The source machine `omega psi inspect --machine` names, if any.
    inspection: Option<&'a str>,
    frontier: Option<&'f mut Option<super::typed_frontier::TypedFrontier>>,
    symbol_sources: Option<Arc<source::SourceMap>>,
    build_accounting: &'a products::SourceCompletionAccounting,
    /// The usage the unprepared source arrived with; each pass starts from it.
    initial_accounting: build_time_evaluation::SourceCompletionAccounting,
    /// The usage of the latest pass, joined into the failure on refusal.
    accounting: &'a mut build_time_evaluation::SourceCompletionAccounting,
    timings: &'a mut artifacts::compile_timings::CompileTimings,
}

/// One preparation and judgment of the source, laid out for the dependency
/// defaults `chosen_defaults` names.
struct JudgedSource {
    program: Arc<typed_trees::FrozenTypedTrees>,
    source_completion_accounting: products::SourceCompletionAccounting,
    judgment: crate::checked::SupplyJudgment,
    provider_defaults: Vec<build_time_evaluation::provider_declarations::TargetProviderDefaults>,
}

fn prepare_and_judge(
    pass: &mut ProductSourcePass<'_, '_>,
    typed: typed_trees::TypedTrees,
    chosen_defaults: &[crate::checked::ChosenDefaultLayout],
) -> Result<JudgedSource, Vec<Diagnostic>> {
    let build = &pass.computed_build_config.config;
    let layout_scopes = layout_target_scopes(&typed, build, chosen_defaults)?;
    let external_roots = build.program_roots();
    let bindings = super::entry_shapes::bound_program_entry_shapes(&typed, &build.root_bindings)?;
    // The Build's own provider selections are supplies of this compilation
    // whether or not the program reaches their slots: each chosen provider
    // is judged against the slot it names. A target package's defaults join
    // below, when a target chooses them for a requirement its entries reach
    // (`judge_supplied_product`).
    let supplies = build
        .provider_selections
        .iter()
        .flat_map(crate::checked::selected_supplies)
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    for scope in &layout_scopes {
        selected.extend(scope.entry_declarations.iter().copied());
    }
    // A library is judged where it is demanded, not wholesale. Every native
    // supply this package's own Build declares needs its exact boundary and
    // signature for calling-policy validation and trust disclosure, even
    // when its target or requirement is unused, so those declarations are
    // claimed here. A dependency's bindings are not: like its provider
    // defaults, they are reached only through the supplies a target chooses
    // for this package's entries (`judge_supplied_product`), where the
    // provider's own calls read each requirement it uses.
    let root = pass.package_inputs.map(PackageCompilationInputs::root);
    selected.extend(
        build
            .native_bindings
            .iter()
            .filter(|supply| root.is_none_or(|root| supply.selection.supplier_package == root))
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
    let source_admission = source_compiler::SourceAdmission {
        invocation_sources: None,
        // Roots belong to structural judgment below, rather than to a deleted
        // source-checking admission or projection.
        external_roots: &[],
        layout_scopes: &layout_scopes,
        layout_selection_authority: pass.package_inputs.map(|inputs| {
            Arc::new(inputs.clone()) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
        representations: source_compiler::ProductRepresentationInput {
            activation: pass.computed_build_config.selected_build_machine_symbol,
            declared: &pass.computed_build_config.declared_product_selections,
            executed: &pass.computed_build_config.product_selection_requests,
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

    let subject = pass.package_inputs.map(PackageCompilationInputs::root);
    let inspection_root = pass
        .inspection
        .map(|name| inspection_root(&typed, subject, name))
        .transpose()?;
    if inspection_root.is_some() {
        // The artifact names the inspected machine. Still check the Build's
        // bound entries and arrivals, and settle supply from those entries.
        selected.extend(external_roots.iter().copied());
    }
    let roster = products::ProductRoster {
        declaration_metadata: Vec::new(),
        entries: external_roots,
        inspection_root,
        subject,
        selected,
        bindings,
        supplies,
    };
    let source = pass.timings.record_result(SOURCE_PREPARATION, || {
        source_compiler::SourcePreparationRequest {
            program: typed,
            admission: source_admission,
            compilation: roster.compilation(),
            accounting: pass.initial_accounting.clone(),
        }
        .prepare()
    });
    let program = match source {
        Ok(source) => {
            *pass.accounting = source.accounting;
            source.program
        }
        Err(failure) => {
            *pass.accounting = failure.accounting;
            return Err(failure.diagnostics);
        }
    };
    // Build invocations belong to another checked context. Join their
    // costs only after Product's own materialization receipts have been
    // authenticated against this source; they prove no Product value.
    let mut source_completion_accounting = pass.build_accounting.clone();
    source_completion_accounting.append(pass.accounting.clone());
    if let Some(frontier) = pass.frontier.as_deref_mut() {
        *frontier = Some(super::typed_frontier::TypedFrontier {
            typed: program.as_ref().clone(),
            source_completion_accounting: source_completion_accounting.clone(),
            entries: roster.entries.clone(),
            subject,
            selected: roster.selected.clone(),
            bindings: roster.bindings.clone(),
        });
    }
    let judged = products::ProductTerminalPublication::judge(
        Arc::clone(&program),
        roster,
        pass.rollback
            .settle(&pass.computed_build_config.config.settings.optimizations)
            .effective()
            .clone(),
    );
    let terminal_publication = Arc::new(judged.map_err(|mut diagnostics| {
        if let Some(sources) = pass.symbol_sources.as_deref() {
            super::attribute_diagnostics(&mut diagnostics, sources, pass.package_inputs);
        }
        diagnostics
    })?);
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
    // The Product has one module for every target, so the providers each
    // target chooses are judged in it before anything is published. The
    // judgment kept is that module's: the verdict, the demand of each target
    // and the publication are all read from it.
    let judgment = crate::checked::judge_supplied_product(crate::checked::ProductSupplySource {
        publication: terminal_publication,
        build: &build.provider_selections,
        native_bindings: &build.native_bindings,
        defaults: &provider_defaults,
        layout_deferred: &build.default_provider_selections,
        layout_prepared: chosen_defaults,
    })
    .map_err(|mut diagnostics| {
        if let Some(sources) = pass.symbol_sources.as_deref() {
            super::attribute_diagnostics(&mut diagnostics, sources, pass.package_inputs);
        }
        diagnostics
    })?;
    Ok(JudgedSource {
        program,
        source_completion_accounting,
        judgment,
        provider_defaults,
    })
}

/// Resolve the exact authored Product path before preparing or judging its body.
fn inspection_root(
    typed: &typed_trees::TypedTrees,
    subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    path: &str,
) -> Result<symbols::SymbolHandle, Vec<Diagnostic>> {
    let symbols = &typed.symbols;
    let leaf = path.rsplit("::").next().unwrap_or(path);
    let mut selected = None;
    for machine in typed.machines() {
        let symbol = machine.symbol;
        if symbols.name(symbol) != leaf
            || symbols.authored_origin(symbol) != symbol
            || symbols.display_path(symbol, "::") != path
            || !typed_trees::source_subjects::contains(
                typed,
                symbol,
                &[subject],
                source::DependencyScope::Product,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?
        {
            continue;
        }
        if selected.replace(symbol).is_some() {
            return Err(vec![Diagnostic::error(format!(
                "cannot inspect machine `{path}`: the source name is ambiguous"
            ))]);
        }
    }
    selected.ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "cannot inspect machine `{path}`: no declaration has that source name"
        ))]
    })
}

/// The targets whose layouts this source prepares, each with the exact roots
/// whose storage it lays out: the entries bound for that target, the Build's
/// own provider selections for it, and the dependency defaults that target
/// chose (`chosen_defaults`). A default no target chose is never a root, so
/// an imported default cannot request geometry, or add its target, before a
/// target chooses it (wiki/spec/layouts/plans.md#layout-demand-and-validation).
pub(super) fn layout_target_scopes(
    typed: &typed_trees::TypedTrees,
    build: &products::BuildConfig,
    chosen_defaults: &[crate::checked::ChosenDefaultLayout],
) -> Result<Vec<build_time_evaluation::FixedLayoutTargetScope>, Vec<Diagnostic>> {
    let choices = build.provider_selections.iter().collect::<Vec<_>>();
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
    for chosen in chosen_defaults {
        if !profiles.contains(&chosen.target) {
            profiles.push(chosen.target);
        }
    }
    if profiles.is_empty() {
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
        let mut providers = choices
            .iter()
            .filter(|choice| {
                choice
                    .target
                    .is_none_or(|target| target.canonical_name() == profile.target_name())
            })
            .map(|choice| (*choice).clone())
            .collect::<Vec<_>>();
        for chosen in chosen_defaults {
            if chosen.target == profile && !providers.contains(&chosen.selection) {
                providers.push(chosen.selection.clone());
            }
        }
        scopes.push(build_time_evaluation::FixedLayoutTargetScope {
            target: profile.target_name().to_owned(),
            facts: profile.layout_target_facts(),
            entry_declarations: entries,
            provider_declarations: providers,
        });
    }
    Ok(scopes)
}

#[cfg(test)]
mod layout_demand_tests {
    #[test]
    fn an_entryless_product_does_not_demand_every_library_layout() {
        let scopes = super::layout_target_scopes(
            &typed_trees::TypedTrees::default(),
            &products::BuildConfig::default(),
            &[],
        )
        .unwrap();
        assert!(
            scopes
                .iter()
                .all(|scope| scope.entry_declarations.is_empty()),
            "an entryless product needs explicit empty layout roots, not all declarations: {scopes:?}"
        );
    }
}
