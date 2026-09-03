//! Pass-v1 ordered-rule and publication validation.

use super::*;

#[test]
fn pass_record_binds_rule_order_decisions_and_usage() {
    let rules = vec![rule(b"first"), rule(b"second")];
    let rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&rules).unwrap();
    let record = OptimizationPassManifestRecord::new(
        OptimizationPassIdentity::from_canonical_bytes(b"pass"),
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationUnitIdentity::from_canonical_bytes(b"output"),
        rule_set,
        rules.clone(),
        vec![decision(rules[0])],
        OptimizationWorkUsage {
            rule_evaluations: 2,
            candidates: 1,
            validation_steps: 8,
            commits: 1,
            iterations: 1,
        },
    )
    .unwrap();
    assert_eq!(
        OptimizationPassManifestRecord::decode(&record.encode()),
        Ok(record.clone())
    );

    let reversed = vec![rules[1], rules[0]];
    assert_eq!(
        OptimizationPassManifestRecord::new(
            record.pass(),
            record.input(),
            record.output(),
            rule_set,
            reversed,
            Vec::new(),
            OptimizationWorkUsage::default(),
        ),
        Err(InvalidOptimizationManifestRecord::RuleSetIdentityMismatch)
    );
}

#[test]
fn duplicate_and_unscheduled_manifest_identities_reject() {
    let first = rule(b"first");
    assert_eq!(
        OptimizationPassManifestRecord::new(
            OptimizationPassIdentity::from_canonical_bytes(b"pass"),
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationUnitIdentity::from_canonical_bytes(b"output"),
            OptimizationRuleSetIdentity::from_canonical_bytes(b"not-the-rules"),
            vec![first, first],
            Vec::new(),
            OptimizationWorkUsage::default(),
        ),
        Err(InvalidOptimizationManifestRecord::DuplicateRuleIdentity)
    );

    let listed = vec![first];
    let unscheduled = rule(b"unscheduled");
    assert_eq!(
        OptimizationPassManifestRecord::new(
            OptimizationPassIdentity::from_canonical_bytes(b"pass"),
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationUnitIdentity::from_canonical_bytes(b"output"),
            OptimizationRuleSetIdentity::from_ordered_rules(&listed).unwrap(),
            listed,
            vec![
                OptimizationDecisionRecord::new(
                    OptimizationUnitIdentity::from_canonical_bytes(b"input"),
                    OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
                    unscheduled,
                    OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Inapplicable,),
                    AnalysisSet::default(),
                    Vec::new(),
                    None,
                )
                .unwrap()
            ],
            OptimizationWorkUsage::default(),
        ),
        Err(InvalidOptimizationManifestRecord::DecisionNamesUnscheduledRule)
    );
}
