//! Target preparation retains the accepted program's real invocation root.
//! Provider choices are already canonical Psi rows; settlement never reopens
//! source declarations or treats retained provider bodies as separate entries.

use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use products::CheckedProductSource;
use source_compiler::provider_supply::{SourceProviderDemand, SourceProviderFacts};

pub(crate) struct PreparedProductTarget {
    pub(crate) profile: Option<target::TargetProfile>,
    pub(crate) demand: SourceProviderFacts<SourceProviderDemand>,
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
        let config = &source.computed_build_config.config;
        let demand = source.terminal_publication.prepare_provider_demand(
            profile.map(|profile| profile.target_name()),
            &config.provider_selections,
            &config.native_bindings,
            source.sources.package_identity().is_some(),
        )?;
        Ok(Self { profile, demand })
    }
}
