//! Operational custody for the three dead-scalar rules across their two opt-in suites.

use super::super::super::*;

struct Case {
    unit: PsiOptimizationUnit,
    optimization: Optimization,
    iterations: u64,
    rule_evaluations: u64,
    candidates: u64,
    validation_steps: u64,
    rules: Vec<OptimizationRuleIdentity>,
    validators: Vec<optimization_core::OptimizationValidatorIdentity>,
    consumed_fact: Option<OptimizationFactReference>,
    fixed_point_evaluations: u64,
}

fn rule(domain: &[u8]) -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(domain)
}

fn validator(domain: &[u8]) -> optimization_core::OptimizationValidatorIdentity {
    optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(domain)
}

fn cases() -> Vec<Case> {
    let proof_unit = dead_exact_add_unit();
    let proof_fact = proof_unit
        .accepted_obligation_facts
        .first()
        .expect("proof-certified dead scalar fixture carries its accepted fact")
        .identity;
    vec![
        Case {
            unit: dead_scalar_literals_unit(),
            optimization: Optimization::DeadPureScalarElimination,
            iterations: 3,
            rule_evaluations: 4,
            candidates: 3,
            validation_steps: 3,
            rules: vec![
                rule(b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1"),
                rule(b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1"),
            ],
            validators: vec![
                validator(b"omega.validator.dead-unused-scalar-literal.v1"),
                validator(b"omega.validator.dead-unused-scalar-literal.v1"),
            ],
            consumed_fact: None,
            fixed_point_evaluations: 2,
        },
        Case {
            unit: dead_wrapping_add_unit(),
            optimization: Optimization::DeadPureScalarElimination,
            iterations: 4,
            rule_evaluations: 6,
            candidates: 4,
            validation_steps: 4,
            rules: vec![
                rule(b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1"),
                rule(b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1"),
                rule(b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1"),
            ],
            validators: vec![
                validator(b"omega.validator.dead-unused-unconditionally-total-scalar.v1"),
                validator(b"omega.validator.dead-unused-scalar-literal.v1"),
                validator(b"omega.validator.dead-unused-scalar-literal.v1"),
            ],
            consumed_fact: None,
            fixed_point_evaluations: 2,
        },
        Case {
            unit: proof_unit,
            optimization: Optimization::ProofCheckElision,
            iterations: 2,
            rule_evaluations: 13,
            candidates: 1,
            validation_steps: 1,
            rules: vec![rule(
                b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
            )],
            validators: vec![validator(
                b"omega.validator.dead-unused-proof-certified-scalar-node.v1",
            )],
            consumed_fact: Some(OptimizationFactReference::AcceptedObligation(proof_fact)),
            fixed_point_evaluations: 12,
        },
    ]
}

#[test]
fn dead_scalar_rules_are_explicit_deterministic_budgeted_and_idempotent() {
    for case in cases() {
        let disabled = built_in_psi_registry(&OptimizationSelections::default()).unwrap();
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

        let selections = OptimizationSelections::new([case.optimization]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let first = run_unit(case.unit.clone(), &registry, budget(8)).unwrap();
        let second = run_unit(case.unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.2.iterations, case.iterations);
        assert_eq!(first.2.rule_evaluations, case.rule_evaluations);
        assert_eq!(first.2.candidates, case.candidates);
        assert_eq!(first.2.validation_steps, case.validation_steps);
        assert_eq!(first.2.commits, case.rules.len() as u64);
        assert_eq!(first.1.len(), case.rules.len());

        let manifest = first.4.as_ref().expect("selected suite emits a manifest");
        assert_eq!(
            manifest.decisions().len(),
            usize::try_from(case.validation_steps).unwrap()
        );
        let applied_decisions = manifest
            .decisions()
            .iter()
            .filter(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
            .collect::<Vec<_>>();
        assert_eq!(applied_decisions.len(), case.rules.len());
        assert_eq!(first.5.records().len(), case.rules.len());
        for (index, ((commit, decision), record)) in first
            .1
            .iter()
            .zip(applied_decisions)
            .zip(first.5.records())
            .enumerate()
        {
            assert_eq!(commit.rule, case.rules[index]);
            assert_eq!(commit.validator, case.validators[index]);
            assert_eq!(decision.rule(), case.rules[index]);
            assert_eq!(decision.validator(), Some(case.validators[index]));
            assert_eq!(
                decision.consumed_facts(),
                case.consumed_fact.into_iter().collect::<Vec<_>>()
            );
            assert_eq!(record.rule, commit.rule);
            assert_eq!(record.candidate, commit.candidate);
            assert_eq!(record.validator, commit.validator);
            assert_eq!(record.input, commit.input);
            assert_eq!(record.output, commit.output);
            assert_eq!(record.provenance, commit.provenance);
        }
        assert_eq!(manifest.input(), case.unit.identity);
        assert_eq!(manifest.output(), first.0.identity);
        assert_eq!(first.5.input(), case.unit.identity);
        assert_eq!(first.5.output(), first.0.identity);

        let fixed = run_unit(first.0.clone(), &registry, budget(8)).unwrap();
        assert_eq!(fixed.0, first.0);
        assert!(fixed.1.is_empty());
        assert_eq!(fixed.2.iterations, 1);
        assert_eq!(fixed.2.rule_evaluations, case.fixed_point_evaluations);
        assert_eq!(fixed.2.candidates, 0);
        assert_eq!(fixed.2.validation_steps, 0);
        assert_eq!(fixed.2.commits, 0);
        assert!(fixed.3.records.is_empty());
        assert!(fixed.4.unwrap().decisions().is_empty());
        assert!(fixed.5.records().is_empty());

        let first_error = run_unit(case.unit.clone(), &registry, budget(1)).unwrap_err();
        let second_error = run_unit(case.unit, &registry, budget(1)).unwrap_err();
        assert_eq!(first_error, second_error);
        assert_eq!(
            first_error,
            OptimizationRunError::WorkBudgetExhausted("iterations")
        );
    }
}
