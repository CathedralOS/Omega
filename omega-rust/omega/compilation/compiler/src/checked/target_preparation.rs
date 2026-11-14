// Target preparation asks what one target must supply for this Product, and
// the answer comes from the Product's accepted Terminal module: the boundary
// requirements its entries reach through the providers that target chooses,
// paired with the provider selections the Build made and the defaults the
// target package retains (`provider_planning::target_supply`). The module is
// the Product's one module, already judged with the providers every target
// chooses (`judge_supplied_product`, run when the source is checked). A
// target that reaches none prepares an empty supply; a module that demands a
// plan this route cannot form yet is refused by the name of what it demands.
// Typed declarations alone decide nothing here.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::target_supply::{
    JudgedProvider, PreparedTargetSupply, ReachedBoundaryRequirement, TargetSupplyRequest,
};

pub(crate) struct PreparedProductTarget {
    pub(crate) profile: Option<target::TargetProfile>,
    pub(crate) supply: PreparedTargetSupply,
}

/// The slot and provider one selection names, as the roster states a supply.
/// A selection of an operator family names each coordinate as its own slot.
pub(crate) fn selected_supplies(
    selection: &build_time_evaluation::provider_declarations::ProviderSelection,
) -> Vec<psi_compiler::TerminalSourceSupply> {
    use build_time_evaluation::provider_declarations::ProviderSelectionSubject;
    let provider = selection.provider_type.symbol;
    match &selection.subject {
        ProviderSelectionSubject::BoundaryTrait(identity)
        | ProviderSelectionSubject::BoundaryRequirement(identity) => {
            vec![psi_compiler::TerminalSourceSupply {
                slot: identity.symbol,
                provider,
            }]
        }
        ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
            .coordinates()
            .iter()
            .map(|coordinate| psi_compiler::TerminalSourceSupply {
                slot: coordinate.symbol,
                provider,
            })
            .collect(),
    }
}

/// What a Product's accepted module is extended from: the frozen source and
/// the first judgment, with the supplies the Build and the target packages
/// declare.
pub struct ProductSupplySource<'a> {
    /// The frozen source the first judgment accepted.
    pub program: &'a std::sync::Arc<typed_trees::FrozenTypedTrees>,
    /// The roster of that judgment and its accepted module.
    pub roster: &'a compilation_products::ProductRoster,
    pub accepted: &'a psi_compiler::AcceptedTerminalSource,
    /// The evaluated Build's provider selections and the target packages'
    /// defaults.
    pub build: &'a [build_time_evaluation::provider_declarations::ProviderSelection],
    pub defaults: &'a [build_time_evaluation::provider_declarations::TargetProviderDefaults],
}

/// The Product's roster with every chosen provider, and its judgment.
#[derive(Debug)]
pub struct SuppliedProduct {
    /// The Product's own roster, extended by each supply a target chose.
    pub roster: compilation_products::ProductRoster,
    /// The judgment of that roster when a target chose a provider the
    /// Product's own roster did not name; `None` when the first judgment
    /// already holds every chosen provider.
    pub accepted: Option<std::sync::Arc<psi_compiler::AcceptedTerminalSource>>,
}

/// What the demand walk of any target reads from one accepted module.
struct ModuleDemand {
    declared: Vec<ReachedBoundaryRequirement>,
    roots: Vec<semantic_vocabulary::MachineId>,
    judged: Vec<JudgedProvider>,
}

impl ModuleDemand {
    fn read(
        accepted: &psi_compiler::AcceptedTerminalSource,
        roster: &compilation_products::ProductRoster,
    ) -> Result<Self, Vec<Diagnostic>> {
        let declared = accepted
            .boundary_requirements_declared()
            .map_err(|error| vec![error])?
            .into_iter()
            .map(|declared| ReachedBoundaryRequirement {
                boundary: declared.boundary,
                requirement: declared.requirement,
                slot: declared.slot,
                name: declared.name,
            })
            .collect();
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
        accepted: &psi_compiler::AcceptedTerminalSource,
        build: &[build_time_evaluation::provider_declarations::ProviderSelection],
        defaults: &[build_time_evaluation::provider_declarations::TargetProviderDefaults],
        target: Option<&str>,
    ) -> Result<PreparedTargetSupply, Vec<Diagnostic>> {
        PreparedTargetSupply::prepare(TargetSupplyRequest {
            module: accepted.module(),
            declared: &self.declared,
            roots: &self.roots,
            judged: &self.judged,
            build,
            defaults,
            target,
        })
    }
}

/// Has every provider a target chooses judged in the Product's one module.
///
/// The Product has one target-neutral Terminal module, so it holds the
/// providers chosen for every target of the catalog, not one target's
/// (provider_selection.md: "Psi checks every supplied implementation
/// independently of target selection"). Each target's demand is walked from
/// the Product's entries through the providers that target chooses
/// (`provider_planning::target_supply`); each chosen provider joins the
/// roster of the same compilation, where the lowered-form checker judges its
/// machines as roots and proves that each refines the requirement it
/// supplies. A provider may itself reach a boundary requirement, so the
/// roster is judged again until no target chooses anything new; it only
/// grows, and it is bounded by the selections the Build and the target
/// packages declare. A program that reaches no requirement is not judged a
/// second time.
///
/// A target whose demand cannot be prepared, or a requirement no supply can
/// be chosen for, adds nothing here and stops nothing: that is a finding
/// about that target, reported when a product of that target is prepared
/// (`prepare_supplied_target`). A chosen provider the checker does not
/// accept stops the Product on every target, because its source is part of
/// the one module.
pub fn judge_supplied_product(
    source: ProductSupplySource<'_>,
) -> Result<SuppliedProduct, Vec<Diagnostic>> {
    let mut roster = source.roster.clone();
    let mut supplied: Option<std::sync::Arc<psi_compiler::AcceptedTerminalSource>> = None;
    loop {
        let accepted = supplied.as_deref().unwrap_or(source.accepted);
        let demand = ModuleDemand::read(accepted, &roster)?;
        let before = roster.supplies.len();
        for profile in target::TargetProfile::ALL {
            let Ok(supply) = demand.prepare(
                accepted,
                source.build,
                source.defaults,
                Some(profile.target_name()),
            ) else {
                continue;
            };
            for chosen in supply
                .requirements()
                .iter()
                .filter_map(|requirement| requirement.chosen())
                .flat_map(|supply| selected_supplies(&supply.selection))
            {
                if !roster.supplies.contains(&chosen) {
                    roster.supplies.push(chosen);
                }
            }
        }
        if roster.supplies.len() == before {
            return Ok(SuppliedProduct {
                roster,
                accepted: supplied,
            });
        }
        supplied = Some(roster.judge(std::sync::Arc::clone(source.program))?);
    }
}

/// What a product of one target reads from its checked source to prepare
/// that target's supply.
pub struct TargetSupplySource<'a> {
    /// The Product's supplied roster and its accepted module
    /// (`judge_supplied_product`).
    pub roster: &'a compilation_products::ProductRoster,
    pub accepted: &'a psi_compiler::AcceptedTerminalSource,
    /// The evaluated Build's provider selections and the target packages'
    /// defaults.
    pub build: &'a [build_time_evaluation::provider_declarations::ProviderSelection],
    pub defaults: &'a [build_time_evaluation::provider_declarations::TargetProviderDefaults],
    /// The selected deployment, or no deployment for targetless checking.
    pub target: Option<&'a str>,
}

/// Prepares what one target must supply: the requirements its entries reach
/// through the providers it chooses, each with the supplies declared for it.
///
/// Nothing is judged here. A requirement whose chosen provider the accepted
/// module does not hold is refused by name: the walk could not continue
/// through that provider, so what it reaches is unknown, and a product made
/// from the module would call a provider nobody judged.
pub fn prepare_supplied_target(
    source: TargetSupplySource<'_>,
) -> Result<PreparedTargetSupply, Vec<Diagnostic>> {
    let supply = ModuleDemand::read(source.accepted, source.roster)?.prepare(
        source.accepted,
        source.build,
        source.defaults,
        source.target,
    )?;
    let unjudged = supply
        .requirements()
        .iter()
        .filter(|requirement| requirement.realization.is_none())
        .filter_map(|requirement| {
            let chosen = requirement.chosen()?;
            Some(Diagnostic::error(format!(
                "no product route yet: target preparation for `{}`: `{}` is supplied by `{}`, which the accepted module does not hold as a judged provider",
                source.target.unwrap_or("<unscoped>"),
                requirement.reached.name,
                chosen.selection.provider_type.authored_path,
            )))
        })
        .collect::<Vec<_>>();
    if !unjudged.is_empty() {
        return Err(unjudged);
    }
    Ok(supply)
}

impl PreparedProductTarget {
    pub(crate) fn capture(
        source: &CheckedProductSource,
        profile: Option<target::TargetProfile>,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
        source
            .terminal_publication
            .require_source_owner(&source.program)
            .map_err(|error| vec![error])?;
        let supply = prepare_supplied_target(TargetSupplySource {
            roster: source.terminal_publication.roster()?,
            accepted: source.terminal_publication.accepted()?,
            build: &source.computed_build_config.config.provider_selections,
            defaults: &source.provider_defaults,
            target: profile.map(|profile| profile.target_name()),
        })
        .map_err(|mut diagnostics| {
            if let Some(sources) = source.program.symbols.source_map() {
                crate::checked::checking::attribute_diagnostics(
                    &mut diagnostics,
                    &sources,
                    package_inputs,
                );
            }
            diagnostics
        })?;
        Ok(Self { profile, supply })
    }
}
