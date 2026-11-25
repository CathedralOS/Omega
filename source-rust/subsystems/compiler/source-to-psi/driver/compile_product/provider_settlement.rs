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

/// What the demand walk of any target reads from one accepted module.
struct ModuleDemand {
    declared: Vec<ReachedBoundaryRequirement>,
    roots: Vec<semantic_vocabulary::MachineId>,
    judged: Vec<JudgedProvider>,
}

impl ModuleDemand {
    fn read(
        accepted: &production::AcceptedTerminalSource,
        roster: &source_roster::SourceCompilationRoster,
    ) -> Result<Self, Vec<Diagnostic>> {
        let declared = accepted
            .boundary_requirements_declared()
            .map_err(|error| vec![error])?;
        // Every entry the Build binds is a root of every target's walk. The
        // Build binds an entry per target, so a program that binds different
        // entries to different targets is asked on each for what any of
        // them reaches, until the entry of one target is selected here.
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
        accepted: &production::AcceptedTerminalSource,
        build: &[crate::compile_product::provider_declarations::ProviderSelection],
        native_bindings: &[provider_choice_inputs::NativeBuildSupply],
        defaults: &[crate::compile_product::provider_declarations::TargetProviderDefaults],
        target: Option<&str>,
        numeric_support: Option<&language_core::float_operations::NumericTargetCapabilities>,
    ) -> Result<PreparedTargetSupply, Vec<Diagnostic>> {
        PreparedTargetSupply::prepare(TargetSupplyRequest {
            source: accepted.source(),
            module: accepted.module(),
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
    let mut new_supplies = Vec::new();
    let mut unprepared = Vec::new();
    for profile in source.targets {
        let Ok(supply) = demand.prepare(
            accepted,
            source.build,
            source.native_bindings,
            source.defaults,
            Some(profile.numeric_support.target),
            Some(&profile.numeric_support),
        ) else {
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
            if source
                .defaults
                .iter()
                .any(|row| row.selections.contains(selection))
            {
                let layout = ChosenDefaultLayout {
                    target: profile.numeric_support.target.to_owned(),
                    selection: selection.clone(),
                };
                if !source.layout_prepared.contains(&layout) && !unprepared.contains(&layout) {
                    unprepared.push(layout);
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
    if !unprepared.is_empty() {
        let mut layouts = source.layout_prepared.to_vec();
        layouts.append(&mut unprepared);
        return Ok(ProviderWork::PrepareLayouts(layouts));
    }
    if new_supplies.is_empty() {
        return Ok(ProviderWork::Complete);
    }
    Ok(ProviderWork::JudgeProviders(new_supplies))
}
