//! Validated resource-content projections retained for conservation, backing,
//! access-footprint, and artifact consumers.

use arena::HandleSpan;
use language_semantics::content::{ContentConservationPlan, ContentProjectionPlan};
use language_semantics::{PermissionClaimIdentity, ReferenceAccess, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::name::Identifier;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentProjectionFacts {
    pub plans: Vec<ContentProjectionPlan>,
    /// Source-authored n-ary conservation contracts.
    pub conservation_plans: Vec<ContentConservationPlan>,
    /// Checker-derived one-to-one content equalities. Each row is justified by
    /// one exact input-relative claim outcome and never invents a partition
    /// between otherwise independent claims.
    pub identity_reshuffles: Vec<ContentIdentityReshuffleFact>,
    /// Checked wrappers instantiated from an already-authored partition
    /// theorem. These facts retain the exact source theorem, call site,
    /// transfer-stable input claims, and any exact result-identity rewrite rows
    /// used by the substitution; they never add a new `separate(...)` node.
    pub partition_compositions: Vec<ContentPartitionCompositionFact>,
    /// Exact lifetime-bound shared custody admitted for a retained foreign
    /// result. This checked-only row never grants ownership: it binds one
    /// whole direct parameter loan to one whole result and retains both exact
    /// content projections so later consumers need not reselect by spelling
    /// or trust compact report fingerprints.
    pub retained_borrow_custodies: Vec<RetainedBorrowCustodyFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedBorrowCustodyFact {
    pub callable: SymbolHandle,
    pub source: language_semantics::content::ContentStructuralPlace,
    pub result: language_semantics::content::ContentStructuralPlace,
    pub access: ReferenceAccess,
    pub lifetime: Identifier,
    pub callable_lifetime_parameter_ordinal: u32,
    pub result_data: SymbolHandle,
    pub result_lifetime_argument_ordinal: u32,
    pub retained_semantic_domain: SemanticDomainId,
    pub source_projection: ContentProjectionPlan,
    pub result_projection: ContentProjectionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentityReshuffleFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub claim_identity: PermissionClaimIdentity,
    pub input_parameter_symbol: SymbolHandle,
    pub input_segments: HandleSpan<facts::PlaceSegment>,
    pub output_segments: HandleSpan<facts::PlaceSegment>,
    pub plan: ContentConservationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionCompositionFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub source_callable: SymbolHandle,
    /// Non-authoritative compact coordinate for the exact retained source
    /// plan. Lowering replays `source_plan` before publishing this report value.
    pub source_report_fingerprint: u64,
    /// Number of checked wrapper-composition edges between this row's source
    /// theorem and an authored conservation plan. Terminal Psi accepts
    /// only zero because it cannot yet replay a transitive derivation chain.
    pub source_derivation_depth: u32,
    /// The exact theorem before caller-place substitution. Retaining the
    /// source plan lets terminal Psi replay the substitution instead of
    /// trusting a derived `separate(...)` tree in isolation.
    pub source_plan: ContentConservationPlan,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub input_claim_identities: Vec<PermissionClaimIdentity>,
    /// Exact caller-entry place bound to each transferred input claim. This is
    /// independent of any one-to-one output equality and is therefore the
    /// authoritative source for terminal-Psi entry-claim bindings.
    pub input_claim_bindings: Vec<ContentPartitionInputClaimBinding>,
    /// Exact rows proving that staged call-result places reach callable-result
    /// places through identity-preserving local/aggregate transfers. Direct
    /// returns need no such intermediate evidence.
    pub result_rewrites: Vec<ContentPartitionResultRewrite>,
    pub substitutions: Vec<ContentPartitionPlaceSubstitution>,
    pub plan: ContentConservationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionInputClaimBinding {
    pub claim_identity: PermissionClaimIdentity,
    pub entry_place: language_semantics::content::ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionResultRewrite {
    pub claim_identity: PermissionClaimIdentity,
    pub source: language_semantics::content::ContentStructuralPlace,
    pub target: language_semantics::content::ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionPlaceSubstitution {
    pub source: language_semantics::content::ContentStructuralPlace,
    pub target: language_semantics::content::ContentStructuralPlace,
}

impl ContentProjectionFacts {
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
