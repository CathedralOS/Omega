//! Validated resource-content projections retained for conservation, backing,
//! access-footprint, and artifact consumers.

use omega_core::content::{ContentConservationPlan, ContentProjectionPlan};
use omega_core::semantics::SemanticDomainId;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentProjectionFacts {
    pub plans: Vec<ContentProjectionPlan>,
    pub conservation_plans: Vec<ContentConservationPlan>,
}

impl ContentProjectionFacts {
    pub fn for_domain(&self, domain: SymbolHandle) -> Option<&ContentProjectionPlan> {
        self.plans.iter().find(|plan| plan.domain == domain)
    }

    pub fn for_semantic_domain(&self, domain: SemanticDomainId) -> Option<&ContentProjectionPlan> {
        domain
            .is_valid()
            .then(|| {
                self.plans
                    .iter()
                    .find(|plan| plan.semantic_domain == domain)
            })
            .flatten()
    }
}
