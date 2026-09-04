//! Whole-engine operational custody for every exact proof-check-elision roster row.

use super::super::super::*;

struct Case {
    unit: PsiOptimizationUnit,
    rule: OptimizationRuleIdentity,
    validator: omega_optimization_core::OptimizationValidatorIdentity,
    evaluations: u64,
    consumes_constant: bool,
}

fn validator(domain: &[u8]) -> omega_optimization_core::OptimizationValidatorIdentity {
    omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(domain)
}

fn cases() -> Vec<Case> {
    let unsigned = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
    let signed = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 8).unwrap();
    vec![
        Case {
            unit: dead_exact_add_unit(),
            rule: crate::ProofCertifiedDeadScalarEliminationRule::contract().identity(),
            validator: validator(b"omega.validator.dead-unused-proof-certified-scalar-node.v1"),
            evaluations: 13,
            consumes_constant: false,
        },
        Case {
            unit: live_exact_add_zero_unit(),
            rule: crate::LiveProofCertifiedIntegerIdentityEliminationRule::contract().identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-integer-identity-elimination.v1",
            ),
            evaluations: 14,
            consumes_constant: true,
        },
        Case {
            unit: live_divide_by_one_unit(
                unsigned,
                |psi_operation, obligation, result, scalar_type, left, right| {
                    AbstractOperation::ExactIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                },
            ),
            rule: crate::LiveProofCertifiedIntegerDivideByOneEliminationRule::contract().identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1",
            ),
            evaluations: 15,
            consumes_constant: true,
        },
        Case {
            unit: live_exact_multiply_by_zero_unit(unsigned, false),
            rule: crate::LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract()
                .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
            ),
            evaluations: 16,
            consumes_constant: true,
        },
        Case {
            unit: live_zero_dividend_unit(
                unsigned,
                |psi_operation, obligation, result, scalar_type, left, right| {
                    AbstractOperation::ExactIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                },
            ),
            rule: crate::LiveProofCertifiedIntegerZeroDividendEliminationRule::contract()
                .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1",
            ),
            evaluations: 17,
            consumes_constant: true,
        },
        Case {
            unit: live_exact_zero_value_shift_unit(unsigned, true),
            rule: crate::LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule::contract()
                .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
            ),
            evaluations: 18,
            consumes_constant: true,
        },
        Case {
            unit: live_exact_self_subtract_unit(unsigned),
            rule: crate::LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract()
                .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            ),
            evaluations: 19,
            consumes_constant: false,
        },
        Case {
            unit: live_self_remainder_unit(unsigned, SelfRemainderPolicy::Exact),
            rule: crate::LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract()
                .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1",
            ),
            evaluations: 20,
            consumes_constant: false,
        },
        Case {
            unit: live_self_divide_unit(unsigned, SelfDividePolicy::Exact),
            rule: crate::LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1",
            ),
            evaluations: 21,
            consumes_constant: false,
        },
        Case {
            unit: live_remainder_by_one_unit(unsigned, SelfRemainderPolicy::Exact),
            rule: crate::LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract()
                .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1",
            ),
            evaluations: 22,
            consumes_constant: true,
        },
        Case {
            unit: live_signed_remainder_by_negative_one_unit(
                signed,
                SelfRemainderPolicy::Exact,
            ),
            rule:
                crate::LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule::contract()
                    .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
            ),
            evaluations: 23,
            consumes_constant: true,
        },
        Case {
            unit: live_exact_signed_negative_one_shift_right_unit(signed),
            rule:
                crate::LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule::contract()
                    .identity(),
            validator: validator(
                b"omega.validator.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1",
            ),
            evaluations: 24,
            consumes_constant: true,
        },
    ]
}

#[test]
fn every_proof_check_rule_is_explicit_deterministic_budgeted_and_idempotent() {
    let disabled = built_in_psi_registry(&OptimizationSelections::default()).unwrap();
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();

    for case in cases() {
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

        let first = run_unit(case.unit.clone(), &registry, budget(8)).unwrap();
        let second = run_unit(case.unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.2.iterations, 2);
        assert_eq!(first.2.rule_evaluations, case.evaluations);
        assert_eq!(first.2.candidates, 1);
        assert_eq!(first.2.validation_steps, 1);
        assert_eq!(first.2.commits, 1);
        let [commit] = first.1.as_slice() else {
            panic!("one-rule fixture commits exactly once")
        };
        assert_eq!(commit.rule, case.rule);
        assert_eq!(commit.validator, case.validator);
        assert_eq!(commit.input, case.unit.identity);
        assert_eq!(commit.output, first.0.identity);
        assert_eq!(commit.predicted_cost_delta, -1);

        let manifest = first.4.as_ref().expect("selected suite emits a manifest");
        assert_eq!(manifest.ordered_rules().len(), 12);
        let [decision] = manifest.decisions() else {
            panic!("one-rule fixture records exactly one decision")
        };
        assert_eq!(decision.rule(), case.rule);
        assert_eq!(decision.validator(), Some(case.validator));
        assert_eq!(decision.verdict(), OptimizationCandidateVerdict::Applied);
        let accepted = case
            .unit
            .accepted_obligation_facts
            .first()
            .expect("proof-check fixture carries accepted evidence");
        if case.consumes_constant {
            let [
                OptimizationFactReference::ScalarConstant(_),
                OptimizationFactReference::AcceptedObligation(obligation),
            ] = decision.consumed_facts()
            else {
                panic!("constant-backed proof identity consumes constant then obligation")
            };
            assert_eq!(*obligation, accepted.identity);
        } else {
            assert_eq!(
                decision.consumed_facts(),
                [OptimizationFactReference::AcceptedObligation(
                    accepted.identity
                )]
            );
        }

        let [record] = first.5.records() else {
            panic!("one-rule fixture publishes exactly one ledger record")
        };
        assert_eq!(record.rule, commit.rule);
        assert_eq!(record.candidate, commit.candidate);
        assert_eq!(record.validator, commit.validator);
        assert_eq!(record.input, commit.input);
        assert_eq!(record.output, commit.output);
        assert_eq!(record.provenance, commit.provenance);
        assert_eq!(first.5.input(), case.unit.identity);
        assert_eq!(first.5.output(), first.0.identity);
        assert_eq!(
            first.0.accepted_obligation_facts,
            case.unit.accepted_obligation_facts
        );
        assert!(
            first
                .0
                .functions
                .iter()
                .flat_map(|function| &function.facts)
                .all(|fact| !matches!(
                    fact,
                    omega_optimization_unit::OptimizationFact::OperationObligationReference {
                        support,
                        ..
                    } if *support == accepted.operation
                ))
        );

        let fixed = run_unit(first.0.clone(), &registry, budget(8)).unwrap();
        assert_eq!(fixed.0, first.0);
        assert!(fixed.1.is_empty());
        assert_eq!(fixed.2.iterations, 1);
        assert_eq!(fixed.2.rule_evaluations, 12);
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
