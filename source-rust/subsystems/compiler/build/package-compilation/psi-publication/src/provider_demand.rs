//! Project the source owner's completed provider choices onto its immutable portable program.
use super::ProductCodeSource;
use ::provider_demand::IssuedProviderDemand;
use host_provider_inputs::ProviderExecutionOrigins;
use checked_program_to_psi::provider_supply::{
    SourceNativeBinding, SourceProviderDemand, SourceProviderFacts,
};
use checked_program_to_psi::{PortableTerminalProgram, PsiCompilationObserver};
use diagnostics::Diagnostic;
use std::sync::Arc;

/// The issuer of provider demand for a received source judgment. Only this
/// module can supply an instance to the source-free handoff constructor.
#[derive(Debug, PartialEq, Eq)]
pub struct PortableDemandIssuer(typed_trees::source_state::ProductSourceIdentity);

impl PortableDemandIssuer {
    pub fn matches_source(
        &self,
        source: &typed_trees::source_state::ProductSourceIdentity,
    ) -> bool {
        self.0.same_source(source)
    }
}

/// A caller may inspect or consume these facts but cannot replace their
/// roots, bindings or permission origins and publish them again.
///
/// ```compile_fail
/// fn replace_roots(owner: typed_trees::source_state::ProductSourceIdentity, facts: provider_demand::ProviderDemandInputs) -> portable_publication::PublishedProviderDemand {
///     provider_demand::IssuedProviderDemand::issue(portable_publication::PortableDemandIssuer(owner), facts)
/// }
/// ```
///
/// ```compile_fail
/// fn reuse_receipt(demand: portable_publication::PublishedProviderDemand, replacement: provider_demand::ProviderDemandInputs) -> portable_publication::PublishedProviderDemand {
///     let (_, receipt) = demand.into_parts();
///     provider_demand::IssuedProviderDemand::issue(receipt.publisher(), replacement)
/// }
/// ```
pub type PublishedProviderDemand = IssuedProviderDemand<PortableDemandIssuer>;

fn capture_origins(
    program: &PortableTerminalProgram,
    sources: &package_compilation::CheckedSourceCustody,
    profile: target::TargetProfile,
    bindings: Vec<SourceNativeBinding>,
) -> ProviderExecutionOrigins {
    ProviderExecutionOrigins::from_inputs(
        Arc::clone(program.artifact()),
        profile,
        host_provider_inputs::ProviderInstallationInputs {
            bindings,
            package_permissions: sources
                .dependency_closure()
                .map(|graph| graph.declaration_permissions()),
            semantic_bindings: Arc::from(sources.accepted_semantic_bindings()),
        },
    )
}

impl ProductCodeSource<'_> {
    pub fn publish_provider_demand(
        &self,
        profile: Option<target::TargetProfile>,
        demand: SourceProviderFacts<SourceProviderDemand>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<PublishedProviderDemand, Vec<Diagnostic>> {
        let facts = demand.receive(
            self.publication,
            profile.map(|profile| profile.target_name()),
        )?;
        let program = self.publish(self.publication.optimizations(), observer)?;
        let issuer = PortableDemandIssuer(self.sources.source_identity().clone());
        PublishedProviderDemand::from_publication(
            issuer,
            Arc::clone(program.artifact()),
            profile,
            facts,
            self.sources
                .dependency_closure()
                .map(|graph| graph.declaration_permissions()),
            Arc::from(self.sources.accepted_semantic_bindings()),
        )
    }

    pub fn publish_provider_origins(
        &self,
        profile: target::TargetProfile,
        bindings: SourceProviderFacts<Vec<SourceNativeBinding>>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<ProviderExecutionOrigins, Vec<Diagnostic>> {
        let bindings = bindings.receive(self.publication, Some(profile.target_name()))?;
        let program = self.publish(self.publication.optimizations(), observer)?;
        Ok(capture_origins(&program, self.sources, profile, bindings))
    }
}
