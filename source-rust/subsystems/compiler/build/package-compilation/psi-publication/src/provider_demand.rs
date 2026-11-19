//! Project the source owner's completed provider choices onto its immutable portable program.
use super::ProductCodeSource;
use backend_plan::ProviderDemandInputs;
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
/// fn replace_roots(owner: typed_trees::source_state::ProductSourceIdentity, facts: backend_plan::ProviderDemandInputs) -> portable_publication::PublishedProviderDemand {
///     backend_plan::IssuedProviderDemand::issue(portable_publication::PortableDemandIssuer(owner), facts)
/// }
/// ```
///
/// ```compile_fail
/// fn reuse_receipt(demand: portable_publication::PublishedProviderDemand, replacement: backend_plan::ProviderDemandInputs) -> portable_publication::PublishedProviderDemand {
///     let (_, receipt) = demand.into_parts();
///     backend_plan::IssuedProviderDemand::issue(receipt.publisher(), replacement)
/// }
/// ```
pub type PublishedProviderDemand = backend_plan::IssuedProviderDemand<PortableDemandIssuer>;

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
        match (profile, facts) {
            (None, SourceProviderDemand::Unscoped { roots }) => Ok(PublishedProviderDemand::issue(
                issuer,
                ProviderDemandInputs::Unscoped {
                    artifact: Arc::clone(program.artifact()),
                    roots,
                },
            )),
            (
                Some(profile),
                SourceProviderDemand::Target {
                    closed,
                    labels,
                    bindings,
                    review,
                },
            ) => {
                let (logical, bodies) = closed.into_parts();
                let origins = capture_origins(&program, self.sources, profile, bindings);
                // Non-executing and named inspection products do not require a program entry.
                let folded_entry = backend_plan::FoldedEntryReach::capture(
                    program.artifact().view(),
                    profile.target_name(),
                    &bodies,
                    !program.applications().is_empty(),
                );
                let review =
                    review.map(
                        |facts| backend_plan::package_review::PackageReviewRequirements {
                            numeric: facts.numeric,
                            bindings: facts.bindings,
                        },
                    );
                Ok(PublishedProviderDemand::issue(
                    issuer,
                    ProviderDemandInputs::Target {
                        profile,
                        logical,
                        folded_entry,
                        bodies,
                        labels,
                        origins,
                        review,
                    },
                ))
            }
            _ => Err(vec![Diagnostic::error(
                "closed provider demand has a different deployment scope",
            )]),
        }
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
