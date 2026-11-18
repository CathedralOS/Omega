//! Project the source owner's completed provider choices onto its immutable portable program.
use super::ProductCodeSource;
use backend_plan::{ProviderDemandInputs, ProviderExecutionOrigins};
use diagnostics::Diagnostic;
use source_compiler::provider_supply::{
    SourceNativeBinding, SourceProviderDemand, SourceProviderFacts,
};
use source_compiler::{PortableTerminalProgram, PsiCompilationObserver};
use std::sync::Arc;

/// The issuer of provider demand for a received source judgment. Only this
/// module can supply an instance to the source-free handoff constructor.
#[derive(Debug, PartialEq, Eq)]
pub struct PortableDemandIssuer(source_compiler::source_state::ProductSourceIdentity);

impl PortableDemandIssuer {
    pub fn matches_source(
        &self,
        source: &source_compiler::source_state::ProductSourceIdentity,
    ) -> bool {
        self.0.same_source(source)
    }
}

/// A caller may inspect or consume these facts but cannot replace their
/// roots, bindings or permission origins and publish them again.
///
/// ```compile_fail
/// fn replace_roots(owner: source_compiler::source_state::ProductSourceIdentity, facts: backend_plan::ProviderDemandInputs) -> portable_publication::PublishedProviderDemand {
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
    bindings: Vec<SourceNativeBinding>,
) -> ProviderExecutionOrigins {
    let artifact = program.artifact();
    let schema_custody = program
        .source_selections()
        .ok_or_else(|| "provider policies lost their source selection custody".to_owned())
        .and_then(|selections| {
            let questions = selections
                .questions_for(artifact.view().into())
                .ok_or_else(|| {
                    "source declaration custody belongs to another program or invocation".to_owned()
                })?;
            Ok(
                terminal_installation::selection_questions::ProductSchemaCustody::from_selections(
                    artifact.view().into(),
                    questions,
                ),
            )
        });
    ProviderExecutionOrigins {
        artifact: Arc::clone(artifact),
        bindings,
        schema_custody,
        package_permissions: sources
            .dependency_closure()
            .map(|graph| graph.declaration_permissions()),
        authority_permissions: package_authority_permissions(sources),
    }
}

fn package_authority_permissions(
    sources: &package_compilation::CheckedSourceCustody,
) -> Result<
    (
        terminal_psi::TerminalIdentityTable,
        Vec<effects::ServiceTerminalAuthorityPermission>,
    ),
    String,
> {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut permissions = Vec::new();
    for binding in sources.accepted_semantic_bindings() {
        for permission in binding.terminal_authority_permissions() {
            let requirement = binding
                .terminal_authority_permission_identities()
                .text(permission.requirement_identity())
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    "accepted package permission lost its requirement identity".to_owned()
                })?;
            let mut permission = permission.clone();
            permission.rekey_requirement_identity(identities.intern(requirement));
            if !permissions.contains(&permission) {
                permissions.push(permission);
            }
        }
    }
    Ok((identities.build(), permissions))
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
                let origins = capture_origins(&program, self.sources, bindings);
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
        Ok(capture_origins(&program, self.sources, bindings))
    }
}
