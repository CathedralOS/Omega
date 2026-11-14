// Target preparation asks what one target must supply for this Product, and
// the answer comes from the Product's accepted Terminal module: the boundary
// requirements its entry reaches, paired with the provider selections the
// Build made and the defaults the target package retains
// (`provider_planning::target_supply`). A module that reaches none prepares
// an empty supply; a module that demands a plan this route cannot form yet is
// refused by the name of what it demands. Typed declarations alone decide
// nothing here.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::target_supply::{
    PreparedTargetSupply, ReachedBoundaryRequirement, TargetSupplyRequest,
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

/// What one target must supply, with every chosen provider judged.
#[derive(Debug)]
pub struct SuppliedTarget {
    /// The demand of the last judged module and the supplies declared for
    /// it; settlement chooses from it.
    pub supply: PreparedTargetSupply,
    /// The roster the last judgment accepted: the Product's own, extended
    /// by each chosen supply.
    pub roster: compilation_products::ProductRoster,
    /// The judgment of that roster when settlement chose a provider the
    /// Product's own roster did not name; `None` when the first judgment
    /// already covers every chosen supply.
    pub accepted: Option<std::sync::Arc<psi_compiler::AcceptedTerminalSource>>,
}

/// What a product of one target reads from its checked source to prepare
/// that target's supply.
pub struct TargetSupplySource<'a> {
    /// The frozen source the first judgment accepted.
    pub program: &'a std::sync::Arc<typed_trees::FrozenTypedTrees>,
    /// The roster of that judgment and its accepted module.
    pub roster: &'a compilation_products::ProductRoster,
    pub accepted: &'a psi_compiler::AcceptedTerminalSource,
    /// The evaluated Build's provider selections and the target packages'
    /// defaults.
    pub build: &'a [build_time_evaluation::provider_declarations::ProviderSelection],
    pub defaults: &'a [build_time_evaluation::provider_declarations::TargetProviderDefaults],
    /// The canonical name of the target being prepared.
    pub target: &'a str,
}

/// Prepares what one target must supply and has every chosen provider
/// judged.
///
/// The demand is read from the accepted module. Settlement chooses one
/// supply per reached requirement, and each chosen provider then joins the
/// roster of the same compilation, where the lowered-form checker judges its
/// machines as roots and proves that each refines the requirement it
/// supplies. A provider may itself reach a boundary requirement, so the
/// roster is judged again until settlement chooses nothing new; it only
/// grows, and it is bounded by the selections the Build and the target
/// packages declare. A program that reaches no requirement is not judged a
/// second time.
///
/// The module judged with the chosen providers decides the verdict and the
/// demand. It is not yet what a product publishes: the shared Terminal
/// publication stays the first judgment's module until provider plans are
/// formed from the supplied one.
pub fn prepare_supplied_target(
    source: TargetSupplySource<'_>,
) -> Result<SuppliedTarget, Vec<Diagnostic>> {
    let mut roster = source.roster.clone();
    let mut supplied: Option<std::sync::Arc<psi_compiler::AcceptedTerminalSource>> = None;
    loop {
        let accepted = supplied.as_deref().unwrap_or(source.accepted);
        let reached = accepted
            .boundary_requirements_reached()
            .map_err(|error| vec![error])?
            .into_iter()
            .map(|reached| ReachedBoundaryRequirement {
                boundary: reached.boundary,
                requirement: reached.requirement,
                slot: reached.slot,
                name: reached.name,
            })
            .collect::<Vec<_>>();
        let supply = PreparedTargetSupply::prepare(TargetSupplyRequest {
            module: accepted.module(),
            reached: &reached,
            build: source.build,
            defaults: source.defaults,
            target: source.target,
        })?;
        let before = roster.supplies.len();
        for requirement in supply.clone().settle()?.requirements() {
            for chosen in selected_supplies(&requirement.supply.selection) {
                if !roster.supplies.contains(&chosen) {
                    roster.supplies.push(chosen);
                }
            }
        }
        if roster.supplies.len() == before {
            return Ok(SuppliedTarget {
                supply,
                roster,
                accepted: supplied,
            });
        }
        supplied = Some(roster.judge(std::sync::Arc::clone(source.program))?);
    }
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
        let target = profile.unwrap_or_else(target::TargetProfile::host);
        let supplied = prepare_supplied_target(TargetSupplySource {
            program: &source.program,
            roster: source.terminal_publication.roster()?,
            accepted: source.terminal_publication.accepted()?,
            build: &source.computed_build_config.config.provider_selections,
            defaults: &source.provider_defaults,
            target: target.target_name(),
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
        Ok(Self {
            profile,
            supply: supplied.supply,
        })
    }
}
