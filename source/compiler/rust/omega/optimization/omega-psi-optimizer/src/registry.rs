use std::{collections::BTreeSet, sync::Arc};

use omega_optimization_core::{
    AnalysisKind, OptimizationPassIdentity, OptimizationRuleContract, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity,
};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, PsiRewriteCandidateError};

use crate::AnalysisProduct;

#[derive(Debug, Clone, Copy)]
pub struct RuleAnalysisView<'a> {
    products: &'a [AnalysisProduct],
}

impl<'a> RuleAnalysisView<'a> {
    pub const fn new(products: &'a [AnalysisProduct]) -> Self {
        Self { products }
    }

    pub fn get(self, kind: AnalysisKind) -> Option<&'a AnalysisProduct> {
        self.products.iter().find(|product| product.kind() == kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleProposalError {
    MissingAnalysis(AnalysisKind),
    InvalidCandidate(PsiRewriteCandidateError),
}

impl std::fmt::Display for RuleProposalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Psi optimizer rule proposal failed: {self:?}")
    }
}

impl std::error::Error for RuleProposalError {}

/// Immutable executable rule declaration. Candidate enumeration is added by
/// the rule-engine layer; the registry itself depends only on stable contracts
/// and cannot mutate a compilation or another registry.
pub trait PsiOptimizationRule: std::fmt::Debug + Send + Sync {
    fn contract(&self) -> OptimizationRuleContract;

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleScheduleKey {
    pub pass: OptimizationPassIdentity,
    pub rule: OptimizationRuleIdentity,
}

impl RuleScheduleKey {
    pub const fn from_contract(contract: OptimizationRuleContract) -> Self {
        Self {
            pass: contract.pass(),
            rule: contract.identity(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleRegistryError {
    DuplicateRule(OptimizationRuleIdentity),
    MixedPasses {
        expected: OptimizationPassIdentity,
        actual: OptimizationPassIdentity,
    },
}

impl std::fmt::Display for RuleRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid ordered Psi optimizer registry: {self:?}"
        )
    }
}

impl std::error::Error for RuleRegistryError {}

/// Exact immutable rule schedule owned by one compilation. Construction
/// preserves the pass manager's caller-supplied canonical order instead of
/// sorting opaque identity digests, so schedule order remains meaningful.
#[derive(Debug, Clone)]
pub struct OrderedRuleRegistry {
    rules: Arc<[Arc<dyn PsiOptimizationRule>]>,
    pass: Option<OptimizationPassIdentity>,
    identity: OptimizationRuleSetIdentity,
}

impl OrderedRuleRegistry {
    pub fn new(
        rules: impl IntoIterator<Item = Arc<dyn PsiOptimizationRule>>,
    ) -> Result<Self, RuleRegistryError> {
        let rules = rules.into_iter().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut pass = None;
        let mut identities = Vec::with_capacity(rules.len());
        for rule in &rules {
            let contract = rule.contract();
            let key = RuleScheduleKey::from_contract(contract);
            match pass {
                None => pass = Some(key.pass),
                Some(expected) if expected != key.pass => {
                    return Err(RuleRegistryError::MixedPasses {
                        expected,
                        actual: key.pass,
                    });
                }
                Some(_) => {}
            }
            if !seen.insert(key.rule) {
                return Err(RuleRegistryError::DuplicateRule(key.rule));
            }
            identities.push(key.rule);
        }
        let identity = OptimizationRuleSetIdentity::from_ordered_rules(&identities)
            .map_err(|duplicate| RuleRegistryError::DuplicateRule(duplicate.0))?;
        Ok(Self {
            rules: rules.into(),
            pass,
            identity,
        })
    }

    /// The named pass group implemented by this registry. Empty registries
    /// have no pass and therefore produce no pass-manifest row.
    pub const fn pass(&self) -> Option<OptimizationPassIdentity> {
        self.pass
    }

    pub const fn identity(&self) -> OptimizationRuleSetIdentity {
        self.identity
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &dyn PsiOptimizationRule> {
        self.rules.iter().map(Arc::as_ref)
    }

    pub fn contracts(&self) -> impl ExactSizeIterator<Item = OptimizationRuleContract> + '_ {
        self.iter().map(PsiOptimizationRule::contract)
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{AnalysisInvalidationSet, AnalysisSet, OptimizationSafetyClass};

    use super::*;

    #[derive(Debug)]
    struct TestRule(OptimizationRuleContract);

    impl PsiOptimizationRule for TestRule {
        fn contract(&self) -> OptimizationRuleContract {
            self.0
        }

        fn propose(
            &self,
            _unit: &PsiOptimizationUnit,
            _analyses: RuleAnalysisView<'_>,
        ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
            Ok(Vec::new())
        }
    }

    fn rule(pass: &[u8], rule: &[u8]) -> Arc<dyn PsiOptimizationRule> {
        Arc::new(TestRule(
            OptimizationRuleContract::new(
                OptimizationRuleIdentity::from_canonical_bytes(rule),
                OptimizationPassIdentity::from_canonical_bytes(pass),
                1,
                AnalysisSet::default(),
                AnalysisInvalidationSet::default(),
                OptimizationSafetyClass::StructuralIdentity,
            )
            .unwrap(),
        ))
    }

    fn scheduled_rules() -> Vec<Arc<dyn PsiOptimizationRule>> {
        vec![rule(b"pass-a", b"rule-b"), rule(b"pass-a", b"rule-a")]
    }

    #[test]
    fn exact_canonical_order_and_identity_are_stable() {
        let first = OrderedRuleRegistry::new(scheduled_rules()).unwrap();
        let second = OrderedRuleRegistry::new(scheduled_rules()).unwrap();
        assert_eq!(first.identity(), second.identity());
        assert_eq!(
            first
                .contracts()
                .map(OptimizationRuleContract::identity)
                .collect::<Vec<_>>(),
            second
                .contracts()
                .map(OptimizationRuleContract::identity)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn duplicate_rejects_and_distinct_schedule_order_changes_identity() {
        let duplicate = rule(b"pass-a", b"same");
        assert!(matches!(
            OrderedRuleRegistry::new([duplicate.clone(), duplicate]),
            Err(RuleRegistryError::DuplicateRule(_))
        ));

        let forward = OrderedRuleRegistry::new(scheduled_rules()).unwrap();
        let mut reversed = scheduled_rules();
        reversed.reverse();
        let reversed = OrderedRuleRegistry::new(reversed).unwrap();
        assert_ne!(forward.identity(), reversed.identity());
    }

    #[test]
    fn one_registry_cannot_blur_multiple_named_pass_groups() {
        assert!(matches!(
            OrderedRuleRegistry::new([rule(b"pass-a", b"rule-a"), rule(b"pass-b", b"rule-b")]),
            Err(RuleRegistryError::MixedPasses { .. })
        ));
    }

    #[test]
    fn concurrent_compilations_own_independent_registry_values() {
        let first = OrderedRuleRegistry::new(scheduled_rules()).unwrap();
        let second = OrderedRuleRegistry::new(Vec::new()).unwrap();
        let thread = std::thread::spawn(move || {
            assert_eq!(first.len(), 2);
            first.identity()
        });
        assert!(second.is_empty());
        assert_ne!(thread.join().unwrap(), second.identity());
    }
}
