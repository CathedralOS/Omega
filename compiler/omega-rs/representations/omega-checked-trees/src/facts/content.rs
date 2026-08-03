//! Validated resource-content projections retained for conservation, backing,
//! access-footprint, and artifact consumers.

use omega_core::arena::HandleSpan;
use omega_core::content::{ContentConservationPlan, ContentProjectionPlan};
use omega_core::semantics::{PermissionClaimIdentity, SemanticDomainId};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentProjectionFacts {
    pub plans: Vec<ContentProjectionPlan>,
    /// Source-authored n-ary conservation contracts.
    pub conservation_plans: Vec<ContentConservationPlan>,
    /// Checker-derived one-to-one content equalities. Each row is justified by
    /// one exact input-relative claim outcome and never invents a partition
    /// between otherwise independent claims.
    pub identity_reshuffles: Vec<ContentIdentityReshuffleFact>,
    /// Checked direct wrappers instantiated from an already-authored partition
    /// theorem. These facts retain the exact source theorem, call site, and
    /// transfer-stable input claims used by the substitution; they never add a
    /// new `separate(...)` node.
    pub partition_compositions: Vec<ContentPartitionCompositionFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentityReshuffleFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub claim_identity: PermissionClaimIdentity,
    pub input_parameter_symbol: SymbolHandle,
    pub input_segments: HandleSpan<omega_facts::PlaceSegment>,
    pub output_segments: HandleSpan<omega_facts::PlaceSegment>,
    pub plan: ContentConservationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionCompositionFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub source_callable: SymbolHandle,
    pub source_fingerprint: u64,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub input_claim_identities: Vec<PermissionClaimIdentity>,
    pub plan: ContentConservationPlan,
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
