//! Budget, revision-invalidation, policy-skip, and duplicate fences.

use super::*;

#[test]
fn exhausted_iteration_budget_fails_deterministically_without_output() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let first = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
    let second = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
    assert_eq!(first, second);
    assert_eq!(
        first,
        OptimizationRunError::WorkBudgetExhausted("iterations")
    );
}

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
fn nonprofitable_validated_candidate_is_recorded_as_a_skip() {
    let registry =
        OrderedRuleRegistry::new(
            [Arc::new(NonProfitableExactRule) as Arc<dyn PsiOptimizationRule>],
        )
        .unwrap();
    let (unit, commits, _, decisions, pass_manifest, ledger) =
        run_unit(exact_add_unit(), &registry, budget(2)).unwrap();

    assert!(commits.is_empty());
    assert_eq!(decisions.records.len(), 1);
    assert!(matches!(
        unit.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::ExactIntegerAdd { .. }
    ));
    let manifest = pass_manifest.unwrap();
    assert_eq!(manifest.decisions().len(), 1);
    assert_eq!(
        manifest.decisions()[0].verdict(),
        OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::NotProfitable)
    );
    assert!(manifest.decisions()[0].validator().is_some());
    assert_eq!(manifest.decisions()[0].consumed_facts().len(), 3);
    assert!(ledger.records().is_empty());
    assert_eq!(ledger.input(), ledger.output());
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
