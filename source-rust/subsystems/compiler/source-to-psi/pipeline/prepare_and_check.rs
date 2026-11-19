//! Finish a Product's complete source worklist before native realization.
//! Layout retries and logical provider closure stay inside this owner. A retry
//! uses the original unprepared source, so demand remains lazy and target scoped.

use crate::source_state::service_declarations::{
    PreparedServiceCatalog, provider_declarations::ProviderPlanningContext,
};
use diagnostics::Diagnostic;
use std::sync::Arc;

pub struct ProductSourceInputs<'a> {
    pub targets: &'a [crate::source_targets::SourceTarget],
    pub calling_targets: &'a [&'a str],
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Bindings returned by Build. The receiving source resolves their exact declarations.
    pub root_binding_requests: &'a [crate::declaration_selection::ExecutedRootBinding],
    pub provider_selections: &'a [crate::provider_choice_inputs::ProviderSelection],
    pub default_provider_selections: &'a [crate::provider_choice_inputs::ProviderSelection],
    pub native_bindings: &'a [crate::provider_choice_inputs::NativeBuildSupply],
    pub representations: crate::ProductRepresentationInput<'a>,
    pub layout_selection_authority: Option<Arc<dyn crate::evaluation::BuildTimeSelectionAuthority>>,
    pub wire_demands: &'a [crate::preparation::WireCompatibilityDemand],
    pub optimizations: optimization::PsiOptimizationSelections,
}

pub struct CompletedProductSource {
    pub program: Arc<crate::source_state::FrozenTypedTrees>,
    pub source_completion_accounting: crate::evaluation::SourceCompletionAccounting,
    pub terminal_publication: Arc<crate::SourcePublication>,
}

pub fn prepare_lower_and_check(
    typed: crate::source_state::TypedTrees,
    inputs: ProductSourceInputs<'_>,
    build_accounting: &crate::evaluation::SourceCompletionAccounting,
    accounting: &mut crate::evaluation::SourceCompletionAccounting,
    inspection: Option<&str>,
    frontier: Option<&mut Option<crate::TypedFrontier>>,
    observer: &mut impl crate::PsiCompilationObserver,
) -> Result<CompletedProductSource, Vec<Diagnostic>> {
    accounting
        .validate_charges()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let has_defaults = !inputs.default_provider_selections.is_empty()
        || typed.machines().iter().any(|machine| {
            machine.purpose == crate::source_state::machine::MachinePurpose::ProviderDefaults
        });
    let unprepared = has_defaults.then(|| typed.clone());
    let root_bindings =
        symbol_resolved_trees_to_typed_trees::product_declarations::collect_root_bindings(
            &typed,
            inputs.root_binding_requests,
        )?;
    let mut pass = ProductSourcePass {
        inputs,
        root_bindings,
        inspection,
        frontier,
        build_accounting,
        initial_accounting: accounting.clone(),
        accounting,
        observer,
    };
    let mut judged = prepare_and_judge(&mut pass, typed, &[])?;
    let terminal_publication = loop {
        match judged.judgment {
            crate::source_supply::SupplyJudgment::Supplied(publication) => break publication,
            crate::source_supply::SupplyJudgment::PrepareLayouts(chosen) => {
                let typed = unprepared.clone().ok_or_else(|| vec![Diagnostic::error(
                    "a target chose a provider default absent from the source preparation frontier"
                )])?;
                judged = prepare_and_judge(&mut pass, typed, &chosen)?;
            }
        }
    };
    Ok(CompletedProductSource {
        program: judged.program,
        source_completion_accounting: judged.source_completion_accounting,
        terminal_publication,
    })
}

struct ProductSourcePass<'a, 'f, O> {
    inputs: ProductSourceInputs<'a>,
    root_bindings: Vec<crate::RootBinding>,
    inspection: Option<&'f str>,
    frontier: Option<&'f mut Option<crate::TypedFrontier>>,
    build_accounting: &'f crate::evaluation::SourceCompletionAccounting,
    initial_accounting: crate::evaluation::SourceCompletionAccounting,
    accounting: &'f mut crate::evaluation::SourceCompletionAccounting,
    observer: &'f mut O,
}

struct JudgedSource {
    program: Arc<crate::source_state::FrozenTypedTrees>,
    source_completion_accounting: crate::evaluation::SourceCompletionAccounting,
    judgment: crate::source_supply::SupplyJudgment,
}

pub(crate) fn program_roots(
    bindings: &[crate::RootBinding],
) -> Vec<symbols::SymbolHandle> {
    let mut roots = bindings
        .iter()
        .map(|binding| binding.implementation_symbol)
        .collect::<Vec<_>>();
    roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
    roots.dedup();
    roots
}

fn prepare_and_judge(
    pass: &mut ProductSourcePass<'_, '_, impl crate::PsiCompilationObserver>,
    typed: crate::source_state::TypedTrees,
    chosen_defaults: &[crate::source_supply::ChosenDefaultLayout],
) -> Result<JudgedSource, Vec<Diagnostic>> {
    let build = &pass.inputs;
    let root_bindings = &pass.root_bindings;
    // A library is judged where it is demanded, not wholesale. Every native
    // supply this package's own Build declares needs its exact boundary and
    // signature for calling-policy validation and trust disclosure, even
    // when its target or requirement is unused, so those declarations are
    // claimed here. A dependency's bindings are not: like its provider
    // defaults, they are reached only through the supplies a target chooses
    // for this package's entries (`judge_supplied_product`), where the
    // provider's own calls read each requirement it uses.
    let root = pass.inputs.subject;
    let claimed = build
        .native_bindings
        .iter()
        .filter(|supply| root.is_none_or(|root| supply.selection.supplier_package == root))
        .collect::<Vec<_>>();
    let claimed_selections = claimed
        .iter()
        .map(|supply| &supply.selection)
        .collect::<Vec<_>>();
    let layout_scopes = layout_target_scopes(
        &typed,
        root_bindings,
        build.provider_selections,
        chosen_defaults,
        &claimed_selections,
        build.targets,
    )?;
    let external_roots = program_roots(root_bindings);
    let bindings = crate::entry_shapes::bound_program_entry_shapes(
        &typed,
        root_bindings,
        build.targets,
    )?;
    // The normalized provider application below carries the selected trait arguments.
    let mut selected = Vec::new();
    for scope in &layout_scopes {
        selected.extend(scope.entry_declarations.iter().copied());
    }
    let mut layout_scopes = layout_scopes;
    let subject = pass.inputs.subject;
    if root_bindings.is_empty() {
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
                crate::source_state::source_subjects::contains(
                    &typed,
                    machine,
                    &[subject],
                    source_provenance::DependencyScope::Product,
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
    // Opaque representations enter the source frontier when a value requires them.

    let source_admission = crate::SourceAdmission {
        invocation_sources: None,
        // Roots belong to structural judgment below, rather than to a deleted
        // source-checking admission or projection.
        external_roots: &[],
        layout_scopes: &layout_scopes,
        layout_selection_authority: build.layout_selection_authority.clone(),
        representations: crate::ProductRepresentationInput {
            activation: build.representations.activation,
            declared: build.representations.declared,
            executed: build.representations.executed,
            retained: build.representations.retained,
        },
        wire_demands: build.wire_demands,
        declarations: None,
    };

    let inspection_root = pass
        .inspection
        .map(|name| inspection_root(&typed, subject, name))
        .transpose()?;
    // Entry policies are executable dependencies of the source program. All
    // Build-bound entry contracts join its one roster before targets settle.
    let exact_sources = source_admission::source_identity::toolchain_source_identities(
        typed.symbols.source_files(),
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut declaration_metadata = Vec::new();
    let mut entry_catalog = None;
    for profile in build.targets {
        let slot_name = format!("{}::ProgramEntry", profile.entry.owner);
        if !root_bindings.iter()
            .any(|binding| binding.slot == slot_name)
        {
            continue;
        }
        let catalog = entry_catalog.get_or_insert_with(|| {
            Arc::new(PreparedServiceCatalog::prepare(
                ProviderPlanningContext::new(
                    &typed,
                    source_provenance::DependencyScope::Product,
                ),
                &exact_sources,
            ))
        });
        let catalog = catalog
            .bind(ProviderPlanningContext::new(
                &typed,
                source_provenance::DependencyScope::Product,
            ))
            .map_err(|diagnostic| vec![diagnostic])?;
        let declaration =
            crate::entry_contracts::selected_program_entry_boundary(catalog, profile)?;
        if !declaration_metadata.contains(&declaration) {
            declaration_metadata.push(declaration);
        }
    }
    if inspection_root.is_some() {
        // The artifact names the inspected machine. Still check the Build's
        // bound entries and arrivals, and settle supply from those entries.
        selected.extend(external_roots.iter().copied());
    }
    let mut roster = crate::SourceCompilationRoster {
        declaration_metadata,
        entries: external_roots,
        inspection_root,
        subject,
        selected,
        bindings,
        supplies: Vec::new(),
        normalization: Default::default(),
    };
    let preparation_compilation = roster.preparation_compilation()?;
    let source = pass
        .observer
        .record_result(crate::PsiCompilationStage::SourcePreparation, || {
            crate::SourcePreparationRequest {
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
    let mut provider_defaults = crate::source_provider_choices::retain_provider_defaults(
        ProviderPlanningContext::new(
            &program,
            source_provenance::DependencyScope::Product,
        ),
    )?;
    // Keep every available default for settlement. Only actual target choices
    // become normalized provider records and concrete body judgments here;
    // those choices already entered this pass's exact layout scopes above.
    // Subject declarations and explicit Build supplies are judged separately.
    let prepared_provider_defaults = chosen_defaults
        .iter()
        .map(
            |chosen| crate::provider_choice_inputs::TargetProviderDefaults {
                target: Some(chosen.target.clone()),
                machines: Vec::new(),
                selections: vec![chosen.selection.clone()],
            },
        )
        .collect::<Vec<_>>();
    for selection in build.default_provider_selections {
        provider_defaults.push(
            crate::provider_choice_inputs::TargetProviderDefaults {
                target: selection
                    .target
                    .map(|target| target.canonical_name().to_owned()),
                machines: Vec::new(),
                selections: vec![selection.clone()],
            },
        );
    }
    let crate::source_provider_choices::PreparedProviderChoices {
        selections: provider_inputs,
        declaration_roots,
        calling_applications,
    } = crate::source_provider_choices::prepare_with_source_defaults(
        &program,
        build.calling_targets,
        build.provider_selections,
        &[],
        &prepared_provider_defaults,
    )?;
    roster.selected.extend(declaration_roots);
    let mut entry_contracts = Vec::with_capacity(build.calling_targets.len());
    for name in build.calling_targets {
        let profile = crate::source_targets::target_named(build.targets, name)
            .map_err(|error| vec![error])?;
        if entry_contracts
            .iter()
            .any(|(selected, _, _, _)| *selected == profile.name)
        {
            continue;
        }
        let Some(selected) = selected_program_entry(root_bindings, profile, build.targets)?
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
            .get(selected)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected entry lost its target-resolved source shape",
                )]
            })?
            .implementation;
        let nomination = crate::entry_contracts::select_program_entry_contract(&program, profile)?;
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
        let contract = profile.entry_contract.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "selected entry has no physical contract package",
            )]
        })?;
        let digest = source_provenance::entry_contract_digest(
            contract.manifest_identity,
            contract.relative_source,
            file.source.as_bytes(),
        );
        entry_contracts.push((profile.name, nomination, implementation, digest));
    }
    let entry_inputs = entry_contracts
        .iter()
        .map(
            |(profile, nomination, implementation, digest)| crate::SourceProgramEntryInput {
                target: (*profile).to_owned(),
                application: nomination.application().clone(),
                physical_source_digest: *digest,
                implementation: *implementation,
            },
        )
        .collect::<Vec<_>>();
    let calling_inputs = crate::SourceCallingInputs {
        applications: calling_applications,
        entries: entry_inputs,
    };
    roster.normalization = Arc::new(crate::SourceNormalizationInputs {
        calling: calling_inputs,
        providers: provider_inputs,
    });

    // Build invocations belong to another checked context. Join their
    // costs only after Product's own materialization receipts have been
    // authenticated against this source; they prove no Product value.
    let mut source_completion_accounting = pass.build_accounting.clone();
    source_completion_accounting.append(pass.accounting.clone());
    if let Some(frontier) = pass.frontier.as_deref_mut() {
        *frontier = Some(crate::TypedFrontier {
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
    let judged =
        crate::SourcePublication::judge(Arc::clone(&program), roster, build.optimizations.clone());
    let terminal_publication = Arc::new(judged?);
    // The Product has one module for every target, so the providers each
    // target chooses are judged in it before anything is published. The
    // judgment kept is that module's: the verdict, the demand of each target
    // and the publication are all read from it.
    let deferred_defaults = provider_defaults
        .iter()
        .flat_map(|row| row.selections.iter().cloned())
        .collect::<Vec<_>>();
    let targets = build
        .targets
        .iter()
        .map(|profile| crate::SourceSupplyTarget {
            numeric_support: profile.numeric_support.clone(),
        })
        .collect::<Vec<_>>();
    let judgment =
        crate::source_supply::judge_supplied_product(crate::source_supply::ProductSupplySource {
            targets: &targets,
            publication: terminal_publication,
            build: build.provider_selections,
            native_bindings: build.native_bindings,
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
    typed: &crate::source_state::TypedTrees,
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
            || !crate::source_state::source_subjects::contains(
                typed,
                symbol,
                &[subject],
                source_provenance::DependencyScope::Product,
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
pub fn layout_target_scopes(
    typed: &crate::source_state::TypedTrees,
    root_bindings: &[crate::RootBinding],
    provider_selections: &[crate::provider_choice_inputs::ProviderSelection],
    chosen_defaults: &[crate::source_supply::ChosenDefaultLayout],
    claimed: &[&crate::evaluation::native_bindings::NativeBindingSelection],
    targets: &[crate::source_targets::SourceTarget],
) -> Result<Vec<crate::evaluation::FixedLayoutTargetScope>, Vec<Diagnostic>> {
    let choices = provider_selections.iter().collect::<Vec<_>>();
    let mut profiles = Vec::new();
    for binding in root_bindings {
        let (owner, _) = binding.slot.rsplit_once("::").ok_or_else(|| {
            vec![Diagnostic::error(
                "fixed representation scope has an invalid Build root slot",
            )]
        })?;
        let profile =
            crate::source_targets::target_owner(targets, owner).map_err(|error| vec![error])?;
        if !profiles
            .iter()
            .any(|known: &&crate::source_targets::SourceTarget| known.name == profile.name)
        {
            profiles.push(profile);
        }
    }
    for choice in &choices {
        if let Some(target) = choice.target {
            let profile = crate::source_targets::target_named(targets, target.canonical_name())
                .map_err(|error| vec![error])?;
            if !profiles
                .iter()
                .any(|known: &&crate::source_targets::SourceTarget| known.name == profile.name)
            {
                profiles.push(profile);
            }
        }
    }
    for chosen in chosen_defaults {
        let profile = crate::source_targets::target_named(targets, &chosen.target)
            .map_err(|error| vec![error])?;
        if !profiles
            .iter()
            .any(|known: &&crate::source_targets::SourceTarget| known.name == profile.name)
        {
            profiles.push(profile);
        }
    }
    for selection in claimed {
        let profile =
            crate::source_targets::target_named(targets, selection.target.canonical_name())
                .map_err(|error| vec![error])?;
        if !profiles
            .iter()
            .any(|known: &&crate::source_targets::SourceTarget| known.name == profile.name)
        {
            profiles.push(profile);
        }
    }
    if profiles.is_empty() {
        profiles.extend(targets.iter().filter(|profile| profile.layout.is_some()));
    }
    for machine in typed.machines() {
        for target in [
            typed.machine_authored_target(machine),
            typed.source_target_context(machine),
        ]
        .into_iter()
        .flatten()
        {
            let profile = crate::source_targets::target_named(targets, target)
                .map_err(|error| vec![error])?;
            if !profiles
                .iter()
                .any(|known: &&crate::source_targets::SourceTarget| known.name == profile.name)
            {
                profiles.push(profile);
            }
        }
    }
    let mut scopes = Vec::new();
    for profile in profiles {
        let mut entries = Vec::new();
        for binding in root_bindings {
            let (owner, _) = binding
                .slot
                .rsplit_once("::")
                .expect("root slots were validated above");
            let declared =
                crate::source_targets::target_owner(targets, owner).map_err(|error| vec![error])?;
            if declared.name == profile.name && !entries.contains(&binding.implementation_symbol) {
                entries.push(binding.implementation_symbol);
            }
        }
        for selection in claimed {
            if selection.target.canonical_name() != profile.name {
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
                    .is_none_or(|target| target.canonical_name() == profile.name)
            })
            .map(|choice| (*choice).clone())
            .collect::<Vec<_>>();
        for chosen in chosen_defaults {
            if chosen.target == profile.name && !providers.contains(&chosen.selection) {
                providers.push(chosen.selection.clone());
            }
        }
        scopes.push(crate::evaluation::FixedLayoutTargetScope {
            target: profile.name.to_owned(),
            facts: profile.layout.clone(),
            entry_declarations: entries,
            provider_declarations: providers,
        });
    }
    Ok(scopes)
}

fn selected_program_entry(
    root_bindings: &[crate::RootBinding],
    selected_profile: &crate::source_targets::SourceTarget,
    targets: &[crate::source_targets::SourceTarget],
) -> Result<Option<usize>, Vec<Diagnostic>> {
    if root_bindings.is_empty() {
        return Ok(None);
    }
    let mut diagnostics = Vec::new();
    let mut selected_bindings = Vec::new();
    for (ordinal, binding) in root_bindings.iter().enumerate() {
        let Some((profile, slot_name)) = binding.slot.rsplit_once("::") else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}` is not target-qualified; expected `target::ProgramEntry`",
                binding.slot
            )));
            continue;
        };
        let profile = match crate::source_targets::target_owner(targets, profile) {
            Ok(profile) => profile,
            Err(_) => {
                diagnostics.push(Diagnostic::error(format!(
                    "root slot `{}` belongs to unknown target profile `{profile}`",
                    binding.slot
                )));
                continue;
            }
        };
        let required_slot = &profile.entry;
        if required_slot.slot_name != slot_name {
            diagnostics.push(Diagnostic::error(format!(
                "target profile `{}` declares no required root slot `{}`",
                profile.name, binding.slot
            )));
            continue;
        };
        if profile.name != selected_profile.name {
            continue;
        }
        selected_bindings.push((required_slot, binding, ordinal));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut program_entries = Vec::new();
    for required_slot in [&selected_profile.entry] {
        let matches = selected_bindings
            .iter()
            .filter(|(selected, _, _)| selected == &required_slot)
            .collect::<Vec<_>>();
        let (binding, ordinal) = match matches.as_slice() {
            [(_, binding, ordinal)] => (*binding, *ordinal),
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has no bound required root slot `{}::{}`",
                    selected_profile.name, required_slot.owner, required_slot.slot_name
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has more than one bound required root slot `{}::{}`",
                    selected_profile.name, required_slot.owner, required_slot.slot_name
                )));
                continue;
            }
        };
        let program_entry = required_slot;
        program_entries.push((program_entry, binding, ordinal));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    match program_entries.as_slice() {
        [(_, _, ordinal)] => Ok(Some(*ordinal)),
        [] => Err(vec![Diagnostic::error(format!(
            "selected target `{}` has no supported ProgramEntry root schema",
            selected_profile.name
        ))]),
        _ => Err(vec![Diagnostic::error(format!(
            "selected target `{}` declares more than one ProgramEntry root schema",
            selected_profile.name
        ))]),
    }
}
