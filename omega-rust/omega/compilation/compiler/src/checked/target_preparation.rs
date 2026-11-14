//! Target preparation retains the accepted program's real invocation root.
//! Provider choices are already canonical Psi rows; settlement never reopens
//! source declarations or treats retained provider bodies as separate entries.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::target_supply::{
    JudgedProvider, PreparedTargetSupply, ReachedBoundaryRequirement, TargetSupplyRequest,
};
use semantic_vocabulary::MachineId;

pub(crate) struct PreparedProductTarget {
    pub(crate) profile: Option<target::TargetProfile>,
    pub(crate) entry: Option<MachineId>,
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
    /// The source and complete roster authenticated by one actual judgment.
    pub publication: std::sync::Arc<compilation_products::ProductTerminalPublication>,
    /// The evaluated Build's provider selections and the target packages'
    /// defaults.
    pub build: &'a [build_time_evaluation::provider_declarations::ProviderSelection],
    pub defaults: &'a [build_time_evaluation::provider_declarations::TargetProviderDefaults],
}

/// The existing authenticated publication, extended by every chosen supply.
pub type SuppliedProduct = std::sync::Arc<compilation_products::ProductTerminalPublication>;

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
    let mut publication = source.publication;
    loop {
        let accepted = publication.accepted()?;
        let mut roster = publication.roster()?.clone();
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
            return Ok(publication);
        }
        publication = std::sync::Arc::new(compilation_products::ProductTerminalPublication::judge(
            std::sync::Arc::clone(accepted.source()),
            roster,
            publication.optimizations().clone(),
        )?);
    }
}

/// What a product of one target reads from its checked source to prepare
/// that target's supply.
pub struct TargetSupplySource<'a> {
    /// The Product's source and supplied roster, paired by their judgment.
    pub publication: &'a compilation_products::ProductTerminalPublication,
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
    let accepted = source.publication.accepted()?;
    let supply = ModuleDemand::read(accepted, source.publication.roster()?)?.prepare(
        accepted,
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
        _package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
        source
            .terminal_publication
            .require_source_owner(&source.program)
            .map_err(|error| vec![error])?;
        let accepted = source.terminal_publication.accepted()?;
        let module = accepted.module();
        let entry = match profile {
            Some(profile)
                if !(source.terminal_publication.roster()?.entries.is_empty()
                    && module.program_entry_contracts.is_empty()) =>
            {
                let contract = module
                    .program_entry_contract(profile.target_name())
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                module.machine(contract.entry).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected target entry has no exact executable machine",
                    )]
                })?;
                Some(contract.entry)
            }
            // A reusable source artifact has no executable nomination. It
            // acquires provider demand only when a consumer supplies one.
            _ => None,
        };
        Ok(Self { profile, entry })
    }
}
