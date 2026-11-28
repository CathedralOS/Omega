//! Prepare the exact entries, target contracts and provider declarations that
//! one Product judgment needs. The Product driver owns any subsequent work.
use super::entry_contracts::selected_program_entry;
use super::layout_scopes::layout_target_scopes;
use super::{PreparedProduct, ProductEntry, ProductSourcePass};
use crate::driver::prepare_typed_source;
use crate::source_state::service_declarations::{
    PreparedServiceCatalog, provider_declarations::ProviderPlanningContext,
};
use checked_program_to_psi::provider_choice_inputs;
use diagnostics::Diagnostic;
use prepared_typed_trees_to_lowered_program::source_roster;
use std::sync::Arc;

pub(super) fn program_roots(bindings: &[crate::RootBinding]) -> Vec<symbols::SymbolHandle> {
    let mut roots = bindings
        .iter()
        .map(|binding| binding.implementation_symbol)
        .collect::<Vec<_>>();
    roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
    roots.dedup();
    roots
}

pub(super) fn prepare_roster(
    pass: &mut ProductSourcePass<'_, '_>,
    typed: crate::source_state::TypedTrees,
    chosen_defaults: &[crate::compile_product::provider_settlement::ChosenDefaultLayout],
) -> Result<PreparedProduct, Vec<Diagnostic>> {
    let build = &pass.inputs;
    let nominees = pass.roots.layout_origins(&typed)?;
    // A library is judged where it is demanded, not wholesale. Every native
    // supply this package's own Build declares needs its exact boundary and
    // signature for calling-policy validation and trust disclosure, even
    // when its target or requirement is unused, so those declarations are
    // claimed here. A dependency's bindings are not: like its provider
    // defaults, they are reached only through the supplies a target chooses
    // for this package's entries (`next_provider_work`), where the
    // provider's own calls read each requirement it uses.
    let root = pass.inputs.subject;
    let claimed = pass
        .native_supplies
        .iter()
        .filter(|supply| root.is_none_or(|root| supply.selection.supplier_package == root))
        .collect::<Vec<_>>();
    let claimed_selections = claimed
        .iter()
        .map(|supply| &supply.selection)
        .collect::<Vec<_>>();
    let layout_scopes = layout_target_scopes(
        &typed,
        &nominees,
        &pass.explicit_providers,
        chosen_defaults,
        &claimed_selections,
        build.targets,
        build.calling_targets,
    )?;
    let direct = pass.roots.direct();
    let external_roots = program_roots(direct);
    let bindings = crate::compile_product::entry_shapes::bound_program_entry_shapes(
        &typed,
        direct,
        build.targets,
        build.calling_targets,
    )?;
    // The normalized provider application below carries the selected trait arguments.
    let mut selected = Vec::new();
    for scope in &layout_scopes {
        selected.extend(scope.entry_declarations.iter().copied());
    }
    let subject = pass.inputs.subject;
    let claimed_declarations = || {
        claimed
            .iter()
            .flat_map(|supply| [supply.selection.subject, supply.selection.requirement])
    };
    selected.extend(claimed_declarations());
    // Opaque representations enter the source frontier when a value requires them.

    let source_admission = prepare_typed_source::SourceAdmission {
        invocation_sources: None,
        entry_applications: pass.roots.applications(),
        layout_scopes: &layout_scopes,
        layout_selection_authority: build.declaration_scope.clone(),
        representations: &pass.representations,
    };

    let inspection_root = match pass.entry {
        ProductEntry::Inspection(name) => Some(inspection_root(&typed, subject, name)?),
        ProductEntry::BuildBindings
        | ProductEntry::PackageReview
        | ProductEntry::RegisteredTest { .. } => None,
    };
    // Entry policies are executable dependencies of the selected targets.
    // Inactive bindings still identify real slots but need no physical contract.
    let exact_sources = source_admission::source_identity::toolchain_source_identities(
        typed.symbols.source_files(),
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut declaration_metadata = Vec::new();
    let mut entry_catalog = None;
    for name in build.calling_targets {
        let profile = crate::compile_product::source_targets::target_named(build.targets, name)
            .map_err(|error| vec![error])?;
        let slot_name = format!("{}::ProgramEntry", profile.entry.owner);
        if !nominees.iter().any(|binding| binding.slot == slot_name) {
            continue;
        }
        let catalog = entry_catalog.get_or_insert_with(|| {
            Arc::new(PreparedServiceCatalog::prepare(
                ProviderPlanningContext::new(&typed, source_provenance::DependencyScope::Product),
                &exact_sources,
            ))
        });
        let catalog = catalog
            .bind(ProviderPlanningContext::new(
                &typed,
                source_provenance::DependencyScope::Product,
            ))
            .map_err(|diagnostic| vec![diagnostic])?;
        let declaration = crate::compile_product::entry_contracts::selected_program_entry_boundary(
            catalog, profile,
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
    let mut roster = source_roster::SourceCompilationRoster {
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
        .timings
        .record_result(artifacts::compile_timings::source::PREPARE, || {
            prepare_typed_source::SourcePreparationRequest {
                program: typed,
                admission: source_admission,
                compilation: preparation_compilation,
                accounting: pass.initial_accounting.clone(),
            }
            .prepare()
        });
    let prepared = match source {
        Ok(source) => {
            pass.accounting.append(source.accounting);
            source.program
        }
        Err(failure) => {
            pass.accounting.append(failure.accounting);
            return Err(failure.diagnostics);
        }
    };
    let program = prepared.trees();
    let specialized = pass.roots.specialized(&prepared)?;
    let root_bindings = if let Some(entry) = &specialized {
        // Preparation needs both authored bodies' constants and layouts. The
        // executable closure follows the closed entry's actual calls.
        roster.selected.clear();
        roster.selected.extend(claimed_declarations());
        let bindings = std::slice::from_ref(entry);
        roster.entries = program_roots(bindings);
        roster.bindings = crate::compile_product::entry_shapes::bound_program_entry_shapes(
            program,
            bindings,
            build.targets,
            build.calling_targets,
        )?;
        bindings
    } else {
        direct
    };
    let provider_defaults = product_provider_defaults(program, &pass.default_provider_selections)?;
    // Keep every available default for settlement. Only actual target choices
    // become normalized provider records and concrete body judgments here;
    // those choices already entered this pass's exact layout scopes above.
    // Subject declarations and explicit Build supplies are judged separately.
    let prepared_provider_defaults = chosen_defaults
        .iter()
        .map(|chosen| provider_choice_inputs::TargetProviderDefaults {
            target: Some(chosen.target.clone()),
            machines: Vec::new(),
            selections: vec![chosen.selection.clone()],
        })
        .collect::<Vec<_>>();
    let crate::compile_product::provider_choices::PreparedProviderChoices {
        selections: provider_inputs,
        declaration_roots,
        calling_applications,
    } = crate::compile_product::provider_choices::prepare_with_source_defaults(
        program,
        build.calling_targets,
        &pass.explicit_providers,
        &[],
        &prepared_provider_defaults,
    )?;
    roster.selected.extend(declaration_roots);
    let mut entry_contracts = Vec::with_capacity(build.calling_targets.len());
    for name in build.calling_targets {
        let profile = crate::compile_product::source_targets::target_named(build.targets, name)
            .map_err(|error| vec![error])?;
        if entry_contracts
            .iter()
            .any(|(selected, _, _, _)| *selected == profile.name)
        {
            continue;
        }
        let Some(selected) = selected_program_entry(root_bindings, profile, build.targets)? else {
            // Checking source permits a declaration-only product. Normalize
            // each actual selection here; executable consumers require its
            // checked entry contract when they prepare execution.
            continue;
        };
        // Selection returns the original authored row, including inactive rows
        // in its numbering. Only selected rows have projected calling shapes.
        let implementation = root_bindings[selected].implementation_symbol;
        let nomination = crate::compile_product::entry_contracts::select_program_entry_contract(
            program, profile,
        )?;
        nomination
            .require_owner(program)
            .map_err(|diagnostic| vec![diagnostic])?;
        let contract = profile.entry_contract.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "selected entry has no physical contract package",
            )]
        })?;
        entry_contracts.push((
            profile.name,
            nomination,
            implementation,
            contract.source_digest,
        ));
    }
    let entry_inputs = entry_contracts
        .iter()
        .map(|(profile, nomination, implementation, digest)| {
            source_roster::SourceProgramEntryInput {
                target: (*profile).to_owned(),
                application: nomination.application().clone(),
                physical_source_digest: *digest,
                implementation: *implementation,
            }
        })
        .collect::<Vec<_>>();
    let calling_inputs = source_roster::SourceCallingInputs {
        applications: calling_applications,
        entries: entry_inputs,
    };
    roster.normalization = Arc::new(source_roster::SourceNormalizationInputs {
        calling: calling_inputs,
        providers: provider_inputs,
    });
    Ok(PreparedProduct {
        program: Arc::clone(program),
        source: prepared,
        roster,
        provider_defaults,
    })
}

/// Every default a target may choose: the source's own `provider_defaults`
/// machines, then the defaults dependency Builds retained, one row each.
pub(super) fn product_provider_defaults(
    program: &crate::source_state::TypedTrees,
    retained: &[provider_choice_inputs::ProviderSelection],
) -> Result<Vec<provider_choice_inputs::TargetProviderDefaults>, Vec<Diagnostic>> {
    let mut defaults = crate::compile_product::provider_choices::retain_provider_defaults(
        ProviderPlanningContext::new(program, source_provenance::DependencyScope::Product),
    )?;
    defaults.extend(retained.iter().map(|selection| {
        provider_choice_inputs::TargetProviderDefaults {
            target: selection
                .target
                .map(|target| target.canonical_name().to_owned()),
            machines: Vec::new(),
            selections: vec![selection.clone()],
        }
    }));
    Ok(defaults)
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
