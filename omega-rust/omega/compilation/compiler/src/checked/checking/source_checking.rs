// Prepare the actual frozen typed owner, judge its complete structural closure,
// and retain that same owner for product consumers. No old judgment or checked
// tree projection participates in producing the application.

use artifacts::compile_timings::StageMeta;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use products::{CheckedProductSource, CompileFailure, PreparedProductSource};
use std::sync::Arc;
use typed_trees::service_declarations::{
    PreparedServiceCatalog, provider_declarations::ProviderPlanningContext,
};

const SOURCE_PREPARATION: StageMeta =
    StageMeta::new("typed-source-preparation", "TypedTrees", "FrozenTypedTrees");

pub(crate) fn check_product_source(
    source: PreparedProductSource,
    package_inputs: Option<&PackageCompilationInputs>,
    calling_targets: &[target::TargetProfile],
    rollback: &crate::OptimizationRollback,
    inspection: Option<&str>,
    frontier: Option<&mut Option<super::typed_frontier::TypedFrontier>>,
) -> Result<CheckedProductSource, CompileFailure> {
    let build_accounting = source.build_accounting.clone();
    let symbol_sources = source.typed.symbols.source_map();
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
        // A dependency's provider defaults are laid out only for the targets
        // that choose them, and targets choose from the judged module. The
        // first pass lays out the entries and the Build's own selections. When
        // a target then chooses a default the source was not laid out for, the
        // source is prepared again from this unprepared copy. Both source
        // policy declarations and exported Build rows offer defaults; neither
        // origin makes a default an actual supply before a target chooses it.
        let has_defaults = !build.default_provider_selections.is_empty()
            || typed.machines().iter().any(|machine| {
                machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
            });
        let unprepared = has_defaults.then(|| typed.clone());
        let mut pass = ProductSourcePass {
            computed_build_config: &computed_build_config,
            package_inputs,
            rollback,
            calling_targets,
            inspection,
            frontier,
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
                            "a target chose a provider default absent from the source preparation frontier",
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
            computed_build_config,
            admission,
            sources,
            timings,
        })
    })();
    let mut spent = build_accounting;
    spent.append(accounting);
    result.map_err(|mut diagnostics| {
        if let Some(sources) = symbol_sources.as_deref() {
            super::attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        CompileFailure::source_completion(diagnostics, spent)
    })
}

/// What every preparation of one Product source shares.
struct ProductSourcePass<'a, 'f> {
    computed_build_config: &'a products::ComputedBuildConfig,
    package_inputs: Option<&'a PackageCompilationInputs>,
    rollback: &'a crate::OptimizationRollback,
    calling_targets: &'a [target::TargetProfile],
    /// The source machine `omega psi inspect --machine` names, if any.
    inspection: Option<&'a str>,
    frontier: Option<&'f mut Option<super::typed_frontier::TypedFrontier>>,
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
}

fn prepare_and_judge(
    pass: &mut ProductSourcePass<'_, '_>,
    typed: typed_trees::TypedTrees,
    chosen_defaults: &[crate::checked::ChosenDefaultLayout],
) -> Result<JudgedSource, Vec<Diagnostic>> {
    let build = &pass.computed_build_config.config;
    // A library is judged where it is demanded, not wholesale. Every native
    // supply this package's own Build declares needs its exact boundary and
    // signature for calling-policy validation and trust disclosure, even
    // when its target or requirement is unused, so those declarations are
    // claimed here. A dependency's bindings are not: like its provider
    // defaults, they are reached only through the supplies a target chooses
    // for this package's entries (`judge_supplied_product`), where the
    // provider's own calls read each requirement it uses.
    let root = pass.package_inputs.map(PackageCompilationInputs::root);
    let claimed = build
        .native_bindings
        .iter()
        .filter(|supply| root.is_none_or(|root| supply.selection.supplier_package == root))
        .collect::<Vec<_>>();
    let claimed_selections = claimed
        .iter()
        .map(|supply| &supply.selection)
        .collect::<Vec<_>>();
    let layout_scopes = layout_target_scopes(&typed, build, chosen_defaults, &claimed_selections)?;
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
    let mut layout_scopes = layout_scopes;
    let subject = pass.package_inputs.map(PackageCompilationInputs::root);
    if build.root_bindings.is_empty() {
        // An entryless Check judges every machine its subject declares, so
        // each one demands the layouts its signature and body name
        // (wiki/spec/layouts/plans.md#layout-demand-and-validation). They
        // are layout roots only: the roster still selects nothing extra.
        // A generated machine with no source file is no subject's declaration;
        // leaving it out can only leave a layout unrequested, which refuses.
        let subject_machines = typed
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .filter(|&machine| {
                typed_trees::source_subjects::contains(
                    &typed,
                    machine,
                    &[subject],
                    source::DependencyScope::Product,
                )
                .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        // A scope's own roots here are only its claimed native bindings.
        for scope in &mut layout_scopes {
            for machine in &subject_machines {
                if !scope.entry_declarations.contains(machine) {
                    scope.entry_declarations.push(*machine);
                }
            }
        }
    }
    selected.extend(
        claimed
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

    let inspection_root = pass
        .inspection
        .map(|name| inspection_root(&typed, subject, name))
        .transpose()?;
    // Entry policies are executable dependencies of the source program. All
    // Build-bound entry contracts join its one roster before targets settle.
    let exact_sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut declaration_metadata = Vec::new();
    let mut entry_catalog = None;
    for profile in target::TargetProfile::ALL {
        let slot = profile.program_entry_slot();
        let slot_name = format!("{}::ProgramEntry", profile.root_slot_owner_name());
        if !build
            .root_bindings
            .iter()
            .any(|binding| binding.slot == slot_name)
        {
            continue;
        }
        let catalog = entry_catalog.get_or_insert_with(|| {
            Arc::new(PreparedServiceCatalog::prepare(
                ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
                &exact_sources,
            ))
        });
        let catalog = catalog
            .bind(ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ))
            .map_err(|diagnostic| vec![diagnostic])?;
        let declaration = package_compilation::selected_program_entry_boundary(
            catalog,
            slot,
            pass.package_inputs,
        )?;
        if !declaration_metadata.contains(&declaration) {
            declaration_metadata.push(declaration);
        }
    }
    if inspection_root.is_some() {
        // The artifact names the inspected machine. Still check the Build's
        // bound entries and arrivals, and settle supply from those entries.
        selected.extend(external_roots.iter().copied());
    }
    let mut roster = products::ProductRoster {
        declaration_metadata,
        entries: external_roots,
        inspection_root,
        subject,
        selected,
        bindings,
        supplies,
        normalization: Default::default(),
    };
    let preparation_compilation = roster.preparation_compilation()?;
    let source = pass.timings.record_result(SOURCE_PREPARATION, || {
        source_compiler::SourcePreparationRequest {
            program: typed,
            admission: source_admission,
            compilation: preparation_compilation,
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
    let mut provider_defaults =
        build_time_evaluation::provider_declarations::retain_provider_defaults(
            build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                &program,
                source::DependencyScope::Product,
            ),
        )?;
    // Keep every available default for settlement. Only actual target choices
    // become normalized provider records and concrete body judgments here;
    // those choices already entered this pass's exact layout scopes above.
    // Subject declarations and explicit Build supplies are judged separately.
    let prepared_provider_defaults = chosen_defaults
        .iter()
        .map(
            |chosen| build_time_evaluation::provider_declarations::TargetProviderDefaults {
                target: Some(chosen.target.target_name().to_owned()),
                machines: Vec::new(),
                selections: vec![chosen.selection.clone()],
            },
        )
        .collect::<Vec<_>>();
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
    let super::provider_choices::PreparedProviderChoices {
        selections: provider_inputs,
        declaration_roots,
        calling_applications,
    } = super::provider_choices::prepare_with_source_defaults(
        &program,
        pass.calling_targets,
        &build.provider_selections,
        &[],
        &prepared_provider_defaults,
    )?;
    roster.selected.extend(declaration_roots);
    let mut entry_contracts = Vec::with_capacity(pass.calling_targets.len());
    for &profile in pass.calling_targets {
        if entry_contracts
            .iter()
            .any(|(selected, _, _, _)| *selected == profile)
        {
            continue;
        }
        let Some(selected) = provider_planning::program_entry::selected_program_entry_machine(
            &build.root_bindings,
            Some(profile),
        )?
        else {
            // Checking source permits a declaration-only product. Normalize
            // each actual selection here; executable consumers require its
            // checked entry contract when they prepare execution.
            continue;
        };
        // Shape preparation retains the actual executed binding order and
        // implementation. The slot text is diagnostic text only.
        let implementation = roster
            .bindings
            .get(selected.binding_ordinal)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected entry lost its target-resolved source shape",
                )]
            })?
            .implementation;
        let nomination = package_compilation::source_imports::select_program_entry_contract(
            &program,
            profile,
            pass.package_inputs,
        )?;
        nomination
            .require_owner(&program)
            .map_err(|diagnostic| vec![diagnostic])?;
        let sources = program.symbols.source_map().ok_or_else(|| {
            vec![Diagnostic::error(
                "selected entry contract has no source inventory",
            )]
        })?;
        let file = sources.get(nomination.source()).ok_or_else(|| {
            vec![Diagnostic::error(
                "selected entry contract lost its exact source file",
            )]
        })?;
        let package = profile
            .program_entry_slot()
            .physical_contract_package
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected entry has no physical contract package",
                )]
            })?;
        let digest = *provider_planning::program_entry::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        package, file.source.as_bytes(),
    ).as_bytes();
        entry_contracts.push((profile, nomination, implementation, digest));
    }
    let entry_inputs = entry_contracts
        .iter()
        .map(|(profile, nomination, implementation, digest)| {
            super::typed_frontier::ProgramEntryInput {
                target: profile.target_name().to_owned(),
                application: nomination.application().clone(),
                physical_source_digest: *digest,
                implementation: *implementation,
            }
        })
        .collect::<Vec<_>>();
    let calling_inputs = super::typed_frontier::CallingInputs {
        applications: calling_applications,
        entries: entry_inputs,
    };
    roster.normalization = Arc::new(products::SourceNormalizationInputs {
        calling: calling_inputs,
        providers: provider_inputs,
    });

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
            supplies: roster.supplies.clone(),
            normalization: Arc::clone(&roster.normalization),
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
    let terminal_publication = Arc::new(judged?);
    // The Product has one module for every target, so the providers each
    // target chooses are judged in it before anything is published. The
    // judgment kept is that module's: the verdict, the demand of each target
    // and the publication are all read from it.
    let deferred_defaults = provider_defaults
        .iter()
        .flat_map(|row| row.selections.iter().cloned())
        .collect::<Vec<_>>();
    let judgment = crate::checked::judge_supplied_product(crate::checked::ProductSupplySource {
        publication: terminal_publication,
        build: &build.provider_selections,
        native_bindings: &build.native_bindings,
        defaults: &provider_defaults,
        layout_deferred: &deferred_defaults,
        layout_prepared: chosen_defaults,
    })?;
    Ok(JudgedSource {
        program,
        source_completion_accounting,
        judgment,
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
/// `claimed` are the native bindings this compilation claims: each is a
/// layout root on its own target, because its requirement's signature is
/// read in full (`prepare_and_judge`) and a type there laid out by a policy
/// (`[CLayout<DarwinTimespec>; 2]`) has a layout only for the target the
/// binding names.
pub(super) fn layout_target_scopes(
    typed: &typed_trees::TypedTrees,
    build: &products::BuildConfig,
    chosen_defaults: &[crate::checked::ChosenDefaultLayout],
    claimed: &[&build_time_evaluation::native_bindings::NativeBindingSelection],
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
    for selection in claimed {
        let profile =
            target::TargetProfile::from_canonical_target_name(selection.target.canonical_name())
                .map_err(|error| vec![error])?;
        if !profiles.contains(&profile) {
            profiles.push(profile);
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
        for selection in claimed {
            if selection.target.canonical_name() != profile.target_name() {
                continue;
            }
            for symbol in [selection.subject, selection.requirement] {
                if !entries.contains(&symbol) {
                    entries.push(symbol);
                }
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

    /// A claimed native binding's requirement is read in full, so the types
    /// in its signature are laid out, and only on the target the binding
    /// names: a macOS binding's `[CLayout<DarwinTimespec>; 2]` has macOS
    /// geometry and no other.
    #[test]
    fn a_claimed_native_binding_is_a_layout_root_on_its_own_target() {
        use language_semantics::declaration_selection::{NativeBindingProducer, ProductTarget};
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([3; 32]).unwrap();
        let subject = symbols::SymbolHandle::from_parts(1, 0);
        let requirement = symbols::SymbolHandle::from_parts(2, 0);
        let selection = build_time_evaluation::native_bindings::NativeBindingSelection {
            application: None,
            subject,
            requirement,
            target: ProductTarget::MacosArm64,
            locator: semantic_vocabulary::NativeBindingLocator::Syscall { number: 1 },
            producer: NativeBindingProducer {
                package,
                declaration_path: "build".into(),
                callable_identity: "build::bind_times".into(),
            },
            source_occurrence_digest: [5; 32],
            supplier_package: package,
            source_span: Default::default(),
        };
        let scopes = super::layout_target_scopes(
            &typed_trees::TypedTrees::default(),
            &products::BuildConfig::default(),
            &[],
            &[&selection],
        )
        .unwrap();
        for scope in &scopes {
            let roots = scope.entry_declarations.contains(&subject)
                && scope.entry_declarations.contains(&requirement);
            assert_eq!(roots, scope.target == "macos_arm64", "{scopes:?}");
        }
        assert!(scopes.iter().any(|scope| scope.target == "macos_arm64"));
    }
}
