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
        let target = profile.unwrap_or_else(target::TargetProfile::host);
        let supply = PreparedTargetSupply::prepare(TargetSupplyRequest {
            module: accepted.module(),
            reached: &reached,
            build: &source.computed_build_config.config.provider_selections,
            defaults: &source.provider_defaults,
            target: target.target_name(),
        })?;
        Ok(Self { profile, supply })
    }
}
