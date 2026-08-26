use std::{collections::BTreeSet, sync::Arc};

use omega_optimization_core::{
    OptimizationPassIdentity, OptimizationRuleContract, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity,
};

/// Immutable executable rule declaration. Candidate enumeration is added by
/// the rule-engine layer; the registry itself depends only on stable contracts
/// and cannot mutate a compilation or another registry.
pub trait PsiOptimizationRule: std::fmt::Debug + Send + Sync {
    fn contract(&self) -> OptimizationRuleContract;
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
    identity: OptimizationRuleSetIdentity,
}

impl OrderedRuleRegistry {
    pub fn new(
        rules: impl IntoIterator<Item = Arc<dyn PsiOptimizationRule>>,
    ) -> Result<Self, RuleRegistryError> {
        let rules = rules.into_iter().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut identities = Vec::with_capacity(rules.len());
        for rule in &rules {
            let contract = rule.contract();
            let key = RuleScheduleKey::from_contract(contract);
            if !seen.insert(key.rule) {
                return Err(RuleRegistryError::DuplicateRule(key.rule));
            }
            identities.push(key.rule);
        }
        let identity = OptimizationRuleSetIdentity::from_ordered_rules(&identities)
            .map_err(|duplicate| RuleRegistryError::DuplicateRule(duplicate.0))?;
        Ok(Self {
            rules: rules.into(),
            identity,
        })
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
