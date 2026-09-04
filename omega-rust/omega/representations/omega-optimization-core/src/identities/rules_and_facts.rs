use super::IDENTITY_WIDTH;
use std::collections::BTreeSet;
use std::fmt;

canonical_identity!(
    OptimizationRuleIdentity,
    b"omega.optimization-rule-identity.v1\0"
);
canonical_identity!(
    OptimizationPassIdentity,
    b"omega.optimization-pass-identity.v1\0"
);
canonical_identity!(
    OptimizationCandidateIdentity,
    b"omega.optimization-candidate-identity.v1\0"
);
canonical_identity!(
    ScalarConstantFactIdentity,
    b"omega.scalar-constant-fact-identity.v1\0"
);
canonical_identity!(
    ValueRangeFactIdentity,
    b"omega.value-range-fact-identity.v1\0"
);
canonical_identity!(
    AcceptedObligationFactIdentity,
    b"omega.accepted-obligation-fact-identity.v1\0"
);
canonical_identity!(ProofQuestionIdentity, b"omega.proof-question-identity.v1\0");
canonical_identity!(
    OwnershipFrontierFactIdentity,
    b"omega.ownership-frontier-fact-identity.v1\0"
);
canonical_identity!(
    OptimizationRuleSetIdentity,
    b"omega.optimization-rule-set-identity.v1\0"
);
impl OptimizationRuleSetIdentity {
    /// Bind the complete normalized execution order. Order is meaningful and
    /// therefore is not sorted here; callers must supply the pass manager's
    /// canonical order. Duplicate rule identities are never canonical.
    pub fn from_ordered_rules(
        rules: &[OptimizationRuleIdentity],
    ) -> Result<Self, DuplicateOptimizationRuleIdentity> {
        let mut seen = BTreeSet::new();
        let mut canonical = Vec::with_capacity(8 + rules.len() * IDENTITY_WIDTH);
        canonical.extend_from_slice(
            &u64::try_from(rules.len())
                .expect("ordered optimization rule count fits u64")
                .to_le_bytes(),
        );
        for rule in rules {
            if !seen.insert(*rule) {
                return Err(DuplicateOptimizationRuleIdentity(*rule));
            }
            canonical.extend_from_slice(&rule.bytes());
        }
        Ok(Self::from_canonical_bytes(&canonical))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateOptimizationRuleIdentity(pub OptimizationRuleIdentity);

impl fmt::Display for DuplicateOptimizationRuleIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("normalized optimization rule set contains a duplicate identity")
    }
}

impl std::error::Error for DuplicateOptimizationRuleIdentity {}
