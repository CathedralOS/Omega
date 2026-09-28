//! Budget, revision-invalidation, policy-skip, and duplicate fences.

use super::super::{OptimizationRunError, register_revision, run_unit};
use super::{
    Arc, BTreeMap, DuplicateExactRule, OptimizationUnitIdentity, OrderedRuleRegistry,
    PsiOptimizationRule, budget, exact_add_unit,
};

#[test]
fn synthetic_a_to_b_to_a_revision_cycle_fails_before_repeated_commit() {
    let a = OptimizationUnitIdentity::from_canonical_bytes(b"synthetic-state-a");
    let b = OptimizationUnitIdentity::from_canonical_bytes(b"synthetic-state-b");

    let run = || {
        let mut seen = BTreeMap::from([(a, 0)]);
        let mut committed = Vec::new();
        register_revision(&mut seen, b, 1)?;
        committed.push(b);
        let error = register_revision(&mut seen, a, 2).unwrap_err();
        Ok::<_, OptimizationRunError>((committed, error, seen))
    };

    let first = run().unwrap();
    let second = run().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.0, vec![b]);
    assert_eq!(first.2, BTreeMap::from([(a, 0), (b, 1)]));
    assert_eq!(
        first.1,
        OptimizationRunError::OscillatingRevision {
            identity: a,
            first_seen_iteration: 0,
            repeated_at_iteration: 2,
        }
    );
}

#[test]
fn duplicate_candidate_identity_fails_closed_without_a_manifest() {
    let registry =
        OrderedRuleRegistry::new([Arc::new(DuplicateExactRule) as Arc<dyn PsiOptimizationRule>])
            .unwrap();
    assert!(matches!(
        run_unit(exact_add_unit(), &registry, budget(2)),
        Err(OptimizationRunError::DuplicateCandidate(_))
    ));
}
