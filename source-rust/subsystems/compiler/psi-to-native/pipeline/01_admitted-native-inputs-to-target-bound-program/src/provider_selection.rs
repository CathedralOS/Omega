//! Select the native providers for one published program and target.
use crate::{SelectedExecution, SelectedProviderResult};
use provider_demand::ProviderDemandInputs;
use diagnostics::Diagnostic;

/// The publisher type survives planning and folding. Independent calculations
/// can be inspected without acquiring the portable compiler's issuance.
#[derive(Debug, PartialEq, Eq)]
pub struct PublishedSelection<Publisher, Plans> {
    plans: Plans,
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    publisher: std::sync::Arc<provider_demand::ProviderDemandAuthority<Publisher>>,
}

impl<Publisher, Plans: Clone> Clone for PublishedSelection<Publisher, Plans> {
    fn clone(&self) -> Self {
        Self {
            plans: self.plans.clone(),
            authorization: self.authorization.clone(),
            publisher: self.publisher.clone(),
        }
    }
}

impl<Publisher, Plans> PublishedSelection<Publisher, Plans> {
    pub fn published_authorization(&self) -> &psi_authorization::AuthorizedTerminalArtifact {
        &self.authorization
    }

    pub fn publisher(&self) -> &Publisher {
        self.publisher.publisher()
    }
}

impl<Publisher, Plans> std::ops::Deref for PublishedSelection<Publisher, Plans> {
    type Target = Plans;

    fn deref(&self) -> &Plans {
        &self.plans
    }
}

pub fn select_published_providers<Publisher>(
    demand: native_inputs::AdmittedProviderDemand<Publisher>,
    components: std::collections::BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        std::sync::Arc<component_description::VerifiedComponent>,
    >,
) -> Result<PublishedSelection<Publisher, SelectedExecution>, Vec<Diagnostic>> {
    let (issued, authorization) = demand.into_parts();
    let (facts, publisher) = issued.into_parts();
    Ok(PublishedSelection {
        plans: plan_provider_inputs(facts, components)?,
        authorization,
        publisher,
    })
}

impl<Publisher> PublishedSelection<Publisher, SelectedExecution> {
    pub fn bind_folded_entry(
        &mut self,
        original: &terminal_codec::CanonicalTerminalArtifact,
        evidence: Option<&terminal_codec::EvidenceSeals>,
    ) -> Result<(), String> {
        self.plans.bind_folded_entry(original, evidence)
    }

    pub fn with_folded_program(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            plans: self.plans.with_folded_program(artifact)?,
            authorization: self.authorization.clone(),
            publisher: self.publisher.clone(),
        })
    }

    pub fn into_result(self) -> PublishedSelection<Publisher, SelectedProviderResult> {
        PublishedSelection {
            plans: self.plans.into_result(),
            authorization: self.authorization,
            publisher: self.publisher,
        }
    }
}

fn plan_provider_inputs(
    demand: ProviderDemandInputs,
    components: std::collections::BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        std::sync::Arc<component_description::VerifiedComponent>,
    >,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    use crate::{NativeProgramPlanningError, target_supply::TargetSupplyRealizationError};
    let (profile, logical, bodies, labels, origins, review, has_bound_applications) = match demand {
        ProviderDemandInputs::Unscoped { artifact, roots } => {
            return crate::plan_unscoped_execution(artifact.view(), &roots);
        }
        ProviderDemandInputs::Target {
            profile,
            logical,
            bodies,
            labels,
            origins,
            review,
            has_bound_applications,
        } => (
            profile,
            logical,
            bodies,
            labels,
            origins,
            review,
            has_bound_applications,
        ),
    };
    let artifact = std::sync::Arc::clone(origins.artifact());
    let module = artifact.semantic_module();
    crate::execution::plan_published_target_execution(origins, profile, &logical, &bodies, review, has_bound_applications, components)
        .map_err(|error| match error {
            NativeProgramPlanningError::Supply(TargetSupplyRealizationError::MissingSupplies { target, boundaries }) => {
                boundaries.iter().map(|boundary| {
                    let message = match labels.binary_search_by_key(boundary, |(key, _)| *key) {
                        Ok(position) => format!(
                            "boundary requirement `{}` has no provider on target `{target}`: the Build selects none and the target package declares no default",
                            labels[position].1,
                        ),
                        Err(_) => format!(
                            "reached boundary {} has no normalized chosen provider or exact native leaf on target `{target}`",
                            boundary.get(),
                        ),
                    };
                    Diagnostic::error(message)
                }).collect()
            }
            NativeProgramPlanningError::Supply(error) => vec![Diagnostic::error(error.diagnostic(module))],
            NativeProgramPlanningError::Admission(diagnostics) => diagnostics,
        })
}
