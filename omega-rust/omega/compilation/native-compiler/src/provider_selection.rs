//! Select the native providers for one published program and target.
use backend_plan::ProviderDemandInputs;
use diagnostics::Diagnostic;
pub use native_program_planning::{
    SelectedExecution, SelectedProviderRealization, SelectedProviderResult,
};

/// The publisher type survives planning and folding. Independent calculations
/// can be inspected without acquiring the portable compiler's issuance.
#[derive(Debug, PartialEq, Eq)]
pub struct PublishedSelection<Publisher, Plans> {
    plans: Plans,
    publisher: std::sync::Arc<backend_plan::ProviderDemandAuthority<Publisher>>,
}

impl<Publisher, Plans: Clone> Clone for PublishedSelection<Publisher, Plans> {
    fn clone(&self) -> Self {
        Self {
            plans: self.plans.clone(),
            publisher: self.publisher.clone(),
        }
    }
}

impl<Publisher, Plans> PublishedSelection<Publisher, Plans> {
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
    demand: backend_plan::IssuedProviderDemand<Publisher>,
) -> Result<PublishedSelection<Publisher, SelectedExecution>, Vec<Diagnostic>> {
    let (facts, publisher) = demand.into_parts();
    Ok(PublishedSelection {
        plans: plan_provider_inputs(facts)?,
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
            publisher: self.publisher.clone(),
        })
    }

    pub fn into_result(self) -> PublishedSelection<Publisher, SelectedProviderResult> {
        PublishedSelection {
            plans: self.plans.into_result(),
            publisher: self.publisher,
        }
    }
}

fn plan_provider_inputs(
    demand: ProviderDemandInputs,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    use native_program_planning::{
        NativeProgramPlanningError, target_supply::TargetSupplyRealizationError,
    };
    let (profile, logical, bodies, labels, origins, review, folded_entry) = match demand {
        ProviderDemandInputs::Unscoped { artifact, roots } => {
            return native_program_planning::plan_unscoped_execution(
                artifact.semantic_module(),
                &roots,
            );
        }
        ProviderDemandInputs::Target {
            profile,
            logical,
            bodies,
            labels,
            origins,
            review,
            folded_entry,
        } => (
            profile,
            logical,
            bodies,
            labels,
            origins,
            review,
            folded_entry,
        ),
    };
    let artifact = std::sync::Arc::clone(&origins.artifact);
    let module = artifact.semantic_module();
    native_program_planning::plan_published_target_execution(origins, profile, &logical, &bodies, review, folded_entry)
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
