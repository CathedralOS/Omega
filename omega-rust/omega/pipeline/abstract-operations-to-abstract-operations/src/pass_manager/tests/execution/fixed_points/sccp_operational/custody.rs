use super::super::super::super::*;

const SCCP_RULE_COUNT: usize = 39;

#[derive(Debug)]
pub(super) struct Case {
    pub(super) roster_position: usize,
    pub(super) unit: PsiOptimizationUnit,
    pub(super) rule: OptimizationRuleIdentity,
    pub(super) validator: optimization_core::OptimizationValidatorIdentity,
}

impl Case {
    pub(super) fn integer(
        roster_position: usize,
        unit: PsiOptimizationUnit,
        rule: OptimizationRuleIdentity,
    ) -> Self {
        Self {
            roster_position,
            unit,
            rule,
            validator: optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.exact-integer-evaluation.v2",
            ),
        }
    }

    pub(super) fn boolean(
        roster_position: usize,
        unit: PsiOptimizationUnit,
        rule: OptimizationRuleIdentity,
    ) -> Self {
        Self {
            roster_position,
            unit,
            rule,
            validator: optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.boolean-evaluation.v4",
            ),
        }
    }
}

pub(super) fn assert_operational_custody(cases: Vec<Case>) {
    let disabled = built_in_psi_registry(&OptimizationSelections::default()).unwrap();
    let enabled = built_in_psi_registry(
        &OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap(),
    )
    .unwrap();
    let roster = enabled.contracts().collect::<Vec<_>>();
    assert_eq!(roster.len(), SCCP_RULE_COUNT);

    for case in cases {
        assert_eq!(
            roster[case.roster_position].identity(),
            case.rule,
            "fixture must name its exact SCCP roster row",
        );

        let disabled_run = run_unit(case.unit.clone(), &disabled, budget(8)).unwrap();
        assert_eq!(disabled_run.0, case.unit);
        assert!(disabled_run.1.is_empty());
        assert_eq!(disabled_run.2.iterations, 1);
        assert_eq!(disabled_run.2.rule_evaluations, 0);
        assert_eq!(disabled_run.2.candidates, 0);
        assert_eq!(disabled_run.2.validation_steps, 0);
        assert_eq!(disabled_run.2.commits, 0);
        assert!(disabled_run.3.records.is_empty());
        assert!(disabled_run.4.is_none());
        assert!(disabled_run.5.records().is_empty());
        assert_eq!(disabled_run.5.input(), case.unit.identity);
        assert_eq!(disabled_run.5.output(), case.unit.identity);

        let first = run_unit(case.unit.clone(), &enabled, budget(8)).unwrap();
        let repeated = run_unit(case.unit.clone(), &enabled, budget(8)).unwrap();
        assert_eq!(first, repeated, "SCCP row {}", case.roster_position);
        assert_eq!(first.2.iterations, 2);
        assert_eq!(
            first.2.rule_evaluations,
            u64::try_from(SCCP_RULE_COUNT + case.roster_position + 1).unwrap(),
        );
        assert_eq!(first.2.candidates, 1);
        assert_eq!(first.2.validation_steps, 1);
        assert_eq!(first.2.commits, 1);

        let [commit] = first.1.as_slice() else {
            panic!("isolated SCCP fixture must commit exactly once")
        };
        assert_eq!(commit.rule, case.rule);
        assert_eq!(commit.validator, case.validator);
        assert_eq!(commit.input, case.unit.identity);
        assert_eq!(commit.output, first.0.identity);
        assert_eq!(commit.predicted_cost_delta, -1);
        assert_eq!(commit.declaration.rule(), case.rule);
        assert_eq!(commit.declaration.input(), case.unit.identity);

        let manifest = first.4.as_ref().expect("selected SCCP emits a manifest");
        assert_eq!(manifest.ordered_rules().len(), SCCP_RULE_COUNT);
        assert_eq!(manifest.ordered_rules()[case.roster_position], case.rule,);
        assert_eq!(manifest.input(), case.unit.identity);
        assert_eq!(manifest.output(), first.0.identity);
        let [decision] = manifest.decisions() else {
            panic!("isolated SCCP fixture must publish one decision")
        };
        assert_eq!(decision.rule(), case.rule);
        assert_eq!(decision.validator(), Some(case.validator));
        assert_eq!(decision.verdict(), OptimizationCandidateVerdict::Applied);
        assert_eq!(decision.input(), commit.input);
        assert_eq!(
            decision.consumed_facts(),
            commit.declaration.consumed_facts()
        );
        let expected_fact_count = match commit
            .declaration
            .scalar_evaluation_witness()
            .expect("every SCCP candidate retains typed evaluation evidence")
        {
            optimization_unit::IntegerEvaluationWitness::Unary { .. } => 1,
            optimization_unit::IntegerEvaluationWitness::Binary { .. }
            | optimization_unit::IntegerEvaluationWitness::ProofCertifiedUnary { .. }
            | optimization_unit::IntegerEvaluationWitness::RangeAgainstConstant { .. }
            | optimization_unit::IntegerEvaluationWitness::RangeAgainstRange { .. } => 2,
            optimization_unit::IntegerEvaluationWitness::ProofCertifiedBinary { .. } => 3,
        };
        assert_eq!(decision.consumed_facts().len(), expected_fact_count);

        let [record] = first.5.records() else {
            panic!("isolated SCCP fixture must publish one ledger record")
        };
        assert_eq!(first.5.input(), case.unit.identity);
        assert_eq!(first.5.output(), first.0.identity);
        assert_eq!(record.rule, commit.rule);
        assert_eq!(record.candidate, commit.candidate);
        assert_eq!(record.validator, commit.validator);
        assert_eq!(record.input, commit.input);
        assert_eq!(record.output, commit.output);
        assert_eq!(record.provenance, commit.provenance);

        let fixed = run_unit(first.0.clone(), &enabled, budget(8)).unwrap();
        assert_eq!(fixed.0, first.0);
        assert!(fixed.1.is_empty());
        assert_eq!(fixed.2.iterations, 1);
        assert_eq!(fixed.2.rule_evaluations, SCCP_RULE_COUNT as u64);
        assert_eq!(fixed.2.candidates, 0);
        assert_eq!(fixed.2.validation_steps, 0);
        assert_eq!(fixed.2.commits, 0);
        assert!(fixed.3.records.is_empty());
        assert!(fixed.4.unwrap().decisions().is_empty());
        assert!(fixed.5.records().is_empty());
        assert_eq!(fixed.5.input(), first.0.identity);
        assert_eq!(fixed.5.output(), first.0.identity);

        let first_error = run_unit(case.unit.clone(), &enabled, budget(1)).unwrap_err();
        let repeated_error = run_unit(case.unit, &enabled, budget(1)).unwrap_err();
        assert_eq!(first_error, repeated_error);
        assert_eq!(
            first_error,
            OptimizationRunError::WorkBudgetExhausted("iterations"),
        );
    }
}
