//! Close provider-body demand inside source settlement. Targets supply only
//! declared numeric capabilities; this worklist cannot evaluate native ABI
//! plans or emit code.
use crate::driver::judge_lowered_source;
use checked_program_to_psi as production;
use checked_program_to_psi::provider_choice_inputs;
use checked_program_to_psi::provider_supply::{
    JudgedProvider, PreparedTargetSupply, ReachedBoundaryRequirement, SupplySelection,
    TargetSupplyRequest,
};
use diagnostics::Diagnostic;
use prepared_typed_trees_to_lowered_program::source_roster;

#[cfg(test)]
mod tests;

/// The slot and provider one selection names, as the roster states a supply.
/// A selection of an operator family names each coordinate as its own slot.
pub(crate) fn selected_supplies(
    selection: &crate::compile_product::provider_declarations::ProviderSelection,
) -> Vec<production::TerminalSourceSupply> {
    use crate::compile_product::provider_declarations::ProviderSelectionSubject;
    let provider = selection.provider_type.symbol;
    match &selection.subject {
        ProviderSelectionSubject::BoundaryTrait(identity)
        | ProviderSelectionSubject::BoundaryRequirement(identity) => {
            vec![production::TerminalSourceSupply {
                slot: identity.symbol,
                provider,
            }]
        }
        ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
            .coordinates()
            .iter()
            .map(|coordinate| production::TerminalSourceSupply {
                slot: coordinate.symbol,
                provider,
            })
            .collect(),
    }
}

/// What a Product's lowered module is extended from before running the checker:
/// the frozen source and lowered closure, with the supplies the Build and
/// the target packages declare.
pub(super) struct LoweredSupplySource<'a> {
    pub targets: &'a [super::source_targets::SourceTarget],
    pub lowered: &'a source_roster::LoweredRosterClosure<'a>,
    pub build: &'a [crate::compile_product::provider_declarations::ProviderSelection],
    pub native_bindings: &'a [provider_choice_inputs::NativeBuildSupply],
    pub defaults: &'a [crate::compile_product::provider_declarations::TargetProviderDefaults],
    pub layout_prepared: &'a [ChosenDefaultLayout],
    /// Whether `layout_prepared` came from [`predict_default_layouts`] and no
    /// scan has confirmed it yet.
    pub predicted: bool,
}

/// What a Product's accepted module is extended from: the frozen source and
/// the first judgment, with the supplies the Build and the target packages
/// declare.
pub(super) struct ProductSupplySource<'a> {
    pub targets: &'a [super::source_targets::SourceTarget],
    /// The source and complete roster authenticated by one actual judgment.
    pub publication: &'a judge_lowered_source::SourcePublication,
    /// The evaluated Build's provider selections and the target packages'
    /// defaults.
    pub build: &'a [crate::compile_product::provider_declarations::ProviderSelection],
    pub native_bindings: &'a [provider_choice_inputs::NativeBuildSupply],
    pub defaults: &'a [crate::compile_product::provider_declarations::TargetProviderDefaults],
    /// The deferred defaults the source was laid out for, by target.
    pub layout_prepared: &'a [ChosenDefaultLayout],
}

/// A dependency's default that one target chose, so the source must lay out
/// the default's storage for that target
/// (wiki/spec/layouts/plans.md#layout-demand-and-validation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenDefaultLayout {
    pub target: String,
    pub selection: crate::compile_product::provider_declarations::ProviderSelection,
}

/// Work newly required by one scan of the accepted Product.
#[derive(Debug)]
pub(super) enum ProviderWork {
    /// The current publication contains every provider chosen by a target.
    Complete,
    /// New providers are reachable on the current prepared source.
    JudgeProviders(Vec<production::TerminalSourceSupply>),
    /// A target chose a deferred default whose layouts the source was not
    /// prepared for. The source owner prepares the source again for every
    /// default listed here, those it was already prepared for included, and
    /// judges again. Nothing was judged with the unprepared default.
    PrepareLayouts(Vec<ChosenDefaultLayout>),
}

/// What one scan of a lowered closure asks of the driver. Only a closure
/// lowered from a source prepared for predicted defaults can be mispredicted.
#[derive(Debug)]
pub(super) enum LoweredWork {
    /// Settle as the scan found.
    Settle(ProviderWork),
    /// The source was prepared for predicted defaults, and the targets did
    /// not choose exactly those. The driver discards this source and its
    /// charges and settles from an unprepared source, as if no prediction
    /// had been made.
    Mispredicted,
}

/// One scan's work, with what confirms or refutes a prediction.
struct ProviderScan {
    work: ProviderWork,
    /// Every checked default a target chose, prepared or not.
    chosen_layouts: Vec<ChosenDefaultLayout>,
    /// Whether every target's demand walk ran. A refused walk chooses
    /// nothing, so it cannot confirm a prediction.
    every_target_walked: bool,
}

/// What the demand walk of any target reads from one accepted module.
struct ModuleDemand {
    declared: Vec<ReachedBoundaryRequirement>,
    roots: Vec<semantic_vocabulary::MachineId>,
    judged: Vec<JudgedProvider>,
}

impl ModuleDemand {
    fn read_from_elaboration(
        source: &crate::source_state::FrozenTypedTrees,
        elaboration: &terminal_check_tables::program::Elaboration,
        roster: &source_roster::SourceCompilationRoster,
    ) -> Result<Self, Vec<Diagnostic>> {
        let declared = production::boundary_requirements_declared_for(source, elaboration)
            .map_err(|error| vec![error])?;
        // Every entry the Build binds is a root of every target's walk. The
        // Build binds an entry per target, so a program that binds different
        // entries to different targets is asked on each for what any of
        // them reaches, until the entry of one target is selected here.
        let roots = roster
            .entries
            .iter()
            .map(|entry| {
                elaboration
                    .machines
                    .iter()
                    .find_map(|(machine, symbol)| (*symbol == *entry).then_some(machine))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "no product route yet: the accepted module does not hold a program entry the Build binds",
                        )]
                    })
            })
            .collect::<Result<_, _>>()?;
        let judged = elaboration
            .supplied
            .iter()
            .map(|supplied| JudgedProvider {
                provider: supplied.provider,
                requirement: supplied.requirement,
                machine: supplied.machine,
            })
            .collect();
        Ok(Self {
            declared,
            roots,
            judged,
        })
    }

    fn read(
        accepted: &production::AcceptedTerminalSource,
        roster: &source_roster::SourceCompilationRoster,
    ) -> Result<Self, Vec<Diagnostic>> {
        let declared = accepted
            .boundary_requirements_declared()
            .map_err(|error| vec![error])?;
        let roots = roster
            .entries
            .iter()
            .map(|entry| {
                accepted.machine_of(*entry).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "no product route yet: the accepted module does not hold a program entry the Build binds",
                    )]
                })
            })
            .collect::<Result<_, _>>()?;
        let judged = accepted
            .supplied_requirements()
            .iter()
            .map(|supplied| JudgedProvider {
                provider: supplied.provider,
                requirement: supplied.requirement,
                machine: supplied.machine,
            })
            .collect();
        Ok(Self {
            declared,
            roots,
            judged,
        })
    }

    fn prepare(
        &self,
        source: &crate::source_state::FrozenTypedTrees,
        module: &terminal_psi::TerminalModule,
        build: &[crate::compile_product::provider_declarations::ProviderSelection],
        native_bindings: &[provider_choice_inputs::NativeBuildSupply],
        defaults: &[crate::compile_product::provider_declarations::TargetProviderDefaults],
        target: Option<&str>,
        numeric_support: Option<&language_core::float_operations::NumericTargetCapabilities>,
    ) -> Result<PreparedTargetSupply, Vec<Diagnostic>> {
        PreparedTargetSupply::prepare(TargetSupplyRequest {
            source,
            module,
            declared: &self.declared,
            roots: &self.roots,
            judged: &self.judged,
            build,
            native_bindings,
            defaults,
            target,
            numeric_support,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_provider_work(
    demand: &ModuleDemand,
    source: &crate::source_state::FrozenTypedTrees,
    module: &terminal_psi::TerminalModule,
    roster: &source_roster::SourceCompilationRoster,
    targets: &[super::source_targets::SourceTarget],
    build: &[crate::compile_product::provider_declarations::ProviderSelection],
    native_bindings: &[provider_choice_inputs::NativeBuildSupply],
    defaults: &[crate::compile_product::provider_declarations::TargetProviderDefaults],
    layout_prepared: &[ChosenDefaultLayout],
) -> Result<ProviderScan, Vec<Diagnostic>> {
    let mut new_supplies = Vec::new();
    let mut unprepared = Vec::new();
    let mut chosen_layouts = Vec::new();
    let mut every_target_walked = true;
    for profile in targets {
        let Ok(supply) = demand.prepare(
            source,
            module,
            build,
            native_bindings,
            defaults,
            Some(profile.numeric_support.target),
            Some(&profile.numeric_support),
        ) else {
            every_target_walked = false;
            continue;
        };
        for requirement in supply.requirements() {
            let Some(supply) = requirement.chosen() else {
                continue;
            };
            // Native bindings add no checked body or deferred layout.
            let SupplySelection::Checked(selection) = &supply.selection else {
                continue;
            };
            if defaults
                .iter()
                .any(|row| row.selections.contains(selection))
            {
                let layout = ChosenDefaultLayout {
                    target: profile.numeric_support.target.to_owned(),
                    selection: selection.clone(),
                };
                if !layout_prepared.contains(&layout) && !unprepared.contains(&layout) {
                    unprepared.push(layout.clone());
                }
                if !chosen_layouts.contains(&layout) {
                    chosen_layouts.push(layout);
                }
            }
            // The checked selection already supplied this exact body. Asking
            // again by declaration alone would lose a generic provider's tuple.
            if requirement.realization.is_some() {
                continue;
            }
            for chosen in selected_supplies(selection) {
                if !roster.supplies.contains(&chosen) && !new_supplies.contains(&chosen) {
                    new_supplies.push(chosen);
                }
            }
        }
    }
    let work = if !unprepared.is_empty() {
        let mut layouts = layout_prepared.to_vec();
        layouts.append(&mut unprepared);
        let names = targets
            .iter()
            .map(|profile| profile.numeric_support.target)
            .collect::<Vec<_>>();
        order_layouts(&mut layouts, &names, defaults);
        ProviderWork::PrepareLayouts(layouts)
    } else if new_supplies.is_empty() {
        ProviderWork::Complete
    } else {
        ProviderWork::JudgeProviders(new_supplies)
    };
    Ok(ProviderScan {
        work,
        chosen_layouts,
        every_target_walked,
    })
}

/// Order chosen defaults by target, then by where the target packages
/// declare them. The prepared source then depends only on which defaults the
/// targets chose, not on whether settlement found them in one round or several.
fn order_layouts(
    layouts: &mut [ChosenDefaultLayout],
    targets: &[&str],
    defaults: &[crate::compile_product::provider_declarations::TargetProviderDefaults],
) {
    layouts.sort_by_cached_key(|layout| {
        (
            targets.iter().position(|target| *target == layout.target),
            defaults
                .iter()
                .flat_map(|row| &row.selections)
                .position(|selection| *selection == layout.selection),
        )
    });
}

/// Read every target's choices from a lowered Product before running Stage 06.
/// If lowering produced a formation failure or closure refusal, stop settling
/// so Stage 06 reports that exact refusal unchanged.
///
/// A source prepared for predicted defaults stands only when every target's
/// walk ran and the targets chose exactly the predicted defaults. The source
/// then holds every chosen default's layouts and no other default's, as if
/// settlement had found them one round at a time.
pub(super) fn next_provider_work_from_lowered(
    source: LoweredSupplySource<'_>,
) -> Result<LoweredWork, Vec<Diagnostic>> {
    let Some(elaboration) = source.lowered.closure() else {
        // A refusal from a source prepared for a prediction may belong to a
        // default no target chose; only an unpredicted source reports it.
        if source.predicted {
            return Ok(LoweredWork::Mispredicted);
        }
        return Ok(LoweredWork::Settle(ProviderWork::Complete));
    };
    let roster = source.lowered.roster();
    let trees = source.lowered.source().trees();
    let demand = ModuleDemand::read_from_elaboration(trees, elaboration, roster)?;
    let scan = scan_provider_work(
        &demand,
        trees,
        &elaboration.module,
        roster,
        source.targets,
        source.build,
        source.native_bindings,
        source.defaults,
        source.layout_prepared,
    )?;
    let confirmed = scan.every_target_walked
        && scan.chosen_layouts.len() == source.layout_prepared.len()
        && source
            .layout_prepared
            .iter()
            .all(|layout| scan.chosen_layouts.contains(layout));
    if source.predicted && !confirmed {
        return Ok(LoweredWork::Mispredicted);
    }
    Ok(LoweredWork::Settle(scan.work))
}

/// Read every target's choices from one accepted Product. Missing target-specific
/// supply is reported when that target is requested. A chosen checked provider
/// becomes a root in the shared Product, including all its transitive requirements.
/// Layouts take precedence: the driver must prepare those selected defaults on
/// their target before it can judge the providers on the resulting source.
pub(super) fn next_provider_work(
    source: ProductSupplySource<'_>,
) -> Result<ProviderWork, Vec<Diagnostic>> {
    let accepted = source.publication.accepted()?;
    let roster = source.publication.roster()?;
    let demand = ModuleDemand::read(accepted, roster)?;
    scan_provider_work(
        &demand,
        accepted.source(),
        accepted.module(),
        roster,
        source.targets,
        source.build,
        source.native_bindings,
        source.defaults,
        source.layout_prepared,
    )
    .map(|scan| scan.work)
}

/// Predict the defaults settlement will find a target choosing, so the driver
/// can prepare and lower the source once instead of once per provider level.
///
/// Settlement reads choices from the lowered call graph, which reaches only
/// the bodies already prepared. `Console::write_line` chooses `WindowsConsole`
/// on `windows_x86_64`, and only `WindowsConsole`'s lowered body shows that it
/// calls `WindowsConsoleHost::write`, whose default is a further provider.
/// That takes one preparation and lowering per level. The prediction follows
/// the same chain through the declarations layout demand already reads
/// ([`FixedLayoutTargetScope::demanded_declarations`]): the entries, the
/// Build's providers for the target and each default predicted so far. A
/// default is predicted when those declarations name one of its
/// requirements, no Build selection or native binding outranks it, and no
/// other default competes for the same slot.
///
/// The prediction only orders work. Settlement accepts it when the lowered
/// walk chooses exactly the predicted defaults, and otherwise reports
/// [`LoweredWork::Mispredicted`]; neither a missed nor an extra default can
/// reach the checked program.
pub(super) fn predict_default_layouts(
    program: &crate::source_state::TypedTrees,
    roots: &[symbols::SymbolHandle],
    targets: &[&str],
    build: &[crate::compile_product::provider_declarations::ProviderSelection],
    native_bindings: &[provider_choice_inputs::NativeBuildSupply],
    defaults: &[crate::compile_product::provider_declarations::TargetProviderDefaults],
) -> Vec<ChosenDefaultLayout> {
    use crate::compile_product::provider_declarations::ProviderSelection;
    use crate::compile_time_invocations::FixedLayoutTargetScope;
    let applies = |selection: &ProviderSelection, target: &str| {
        selection
            .target
            .is_none_or(|selected| selected.canonical_name() == target)
    };
    let mut predicted = Vec::new();
    if roots.is_empty() || defaults.iter().all(|row| row.selections.is_empty()) {
        return predicted;
    }
    let dependencies = crate::source_state::references::ComputationDependencyRows::of(program);
    for &target in targets {
        let target_build = build
            .iter()
            .filter(|selection| applies(selection, target))
            .cloned()
            .collect::<Vec<_>>();
        let available = defaults
            .iter()
            .filter(|row| row.target.as_deref().is_none_or(|row| row == target))
            .flat_map(|row| &row.selections)
            .filter(|selection| applies(selection, target))
            .collect::<Vec<_>>();
        let slots = |selection: &ProviderSelection| {
            selected_supplies(selection)
                .into_iter()
                .map(|supply| supply.slot)
                .collect::<Vec<_>>()
        };
        let outranked = |slot: symbols::SymbolHandle| {
            target_build
                .iter()
                .any(|selection| slots(selection).contains(&slot))
                || native_bindings.iter().any(|supply| {
                    supply.selection.target.canonical_name() == target
                        && supply.selection.subject == slot
                })
                || available
                    .iter()
                    .filter(|other| slots(other).contains(&slot))
                    .count()
                    > 1
        };
        let mut chosen: Vec<ProviderSelection> = Vec::new();
        loop {
            let mut scope = FixedLayoutTargetScope {
                target: target.to_owned(),
                facts: None,
                entry_declarations: roots.to_vec(),
                provider_declarations: target_build.clone(),
            };
            scope.provider_declarations.extend(chosen.iter().cloned());
            let Ok(reached) = scope.demanded_declarations(program, &dependencies) else {
                // An unreadable closure predicts nothing more; settlement
                // still finds every choice the slow way.
                break;
            };
            let before = chosen.len();
            for selection in &available {
                if chosen.contains(selection) {
                    continue;
                }
                let named = selected_supplies(selection).into_iter().any(|supply| {
                    !outranked(supply.slot)
                        && supply
                            .requirements(program)
                            .into_iter()
                            .any(|requirement| reached.contains(requirement))
                });
                if named {
                    chosen.push((*selection).clone());
                }
            }
            if chosen.len() == before {
                break;
            }
        }
        predicted.extend(chosen.into_iter().map(|selection| ChosenDefaultLayout {
            target: target.to_owned(),
            selection,
        }));
    }
    order_layouts(&mut predicted, targets, defaults);
    predicted
}
