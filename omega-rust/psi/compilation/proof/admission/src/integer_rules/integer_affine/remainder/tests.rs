use super::*;
use crate::{
    ProofNode, ProofRule, accept_certificate, check_certificate, check_integer_affine_witness,
};
use semantic_vocabulary::ValueId;

fn value(position: u64, carrier: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(position).unwrap(),
        ScalarType::Integer(carrier),
    )
}

fn literal(number: u128, carrier: IntegerType) -> ScalarTerm {
    ScalarTerm::integer(carrier, IntegerValue::Unsigned(number)).unwrap()
}

fn context(carrier: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types((1..=4).map(|position| {
        (
            ValueId::new(position).unwrap(),
            ScalarType::Integer(carrier),
        )
    }))
    .unwrap()
}

fn proof(witness: IntegerAffineWitness, premise: Proposition) -> ProofNode {
    ProofNode {
        conclusion: Proposition::LessThan(witness.target.clone(), witness.root.clone()),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: premise,
                rule: ProofRule::Assumption { index: 0 },
            }),
            witness,
        },
    }
}

#[test]
fn unsigned_runtime_remainder_checks_direct_and_defined_results_at_full_width() {
    for bits in [1, 8, 32, 64, 128] {
        let carrier = IntegerType::new(IntegerSign::Unsigned, bits).unwrap();
        let context = context(carrier);
        let divisor = value(2, carrier);
        let maximum = ScalarTerm::integer(carrier, carrier.maximum_value()).unwrap();
        for dividend in [value(1, carrier), maximum] {
            for expression in [
                ScalarTerm::exact_integer_remainder(carrier, dividend.clone(), divisor.clone()),
                ScalarTerm::wrapping_integer_remainder(carrier, dividend.clone(), divisor.clone()),
                ScalarTerm::saturating_integer_remainder(
                    carrier,
                    dividend.clone(),
                    divisor.clone(),
                ),
            ] {
                let expression = expression.unwrap();
                for named in [false, true] {
                    let target = if named {
                        value(3, carrier)
                    } else {
                        expression.clone()
                    };
                    let axioms = if named {
                        vec![Proposition::Equal(target.clone(), expression.clone())]
                    } else {
                        Vec::new()
                    };
                    let witness = IntegerAffineWitness {
                        root: divisor.clone(),
                        target,
                        definition_axioms: if named { vec![0] } else { Vec::new() },
                        literal_axioms: if named { vec![None] } else { Vec::new() },
                    };
                    for premise in [
                        Proposition::LessThan(literal(0, carrier), divisor.clone()),
                        Proposition::LessOrEqual(literal(1, carrier), divisor.clone()),
                    ] {
                        let proof = proof(witness.clone(), premise.clone());
                        let accepted = accept_certificate(
                            &context,
                            &proof.conclusion,
                            &[premise],
                            &axioms,
                            &proof,
                        )
                        .expect("exact positive divisor bounds the unsigned remainder");
                        assert_eq!(accepted.semantic_axioms.len(), usize::from(named));
                        assert_eq!(accepted.assumptions.len(), 1);
                    }
                }
            }
        }
    }
}

#[test]
fn unsigned_runtime_remainder_rejects_missing_or_changed_positive_premises() {
    let carrier = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let context = context(carrier);
    let divisor = value(2, carrier);
    let witness = IntegerAffineWitness {
        root: divisor.clone(),
        target: ScalarTerm::wrapping_integer_remainder(carrier, value(1, carrier), divisor.clone())
            .unwrap(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let positive = Proposition::LessThan(literal(0, carrier), divisor.clone());
    let valid = proof(witness.clone(), positive.clone());
    assert!(check_certificate(&context, &valid.conclusion, &[], &[], &valid).is_err());
    for premise in [
        Proposition::Truth,
        Proposition::LessOrEqual(literal(0, carrier), divisor.clone()),
        Proposition::LessThan(divisor.clone(), literal(1, carrier)),
        Proposition::LessThan(literal(0, carrier), value(4, carrier)),
        Proposition::Equal(divisor.clone(), literal(0, carrier)),
    ] {
        let invalid = proof(witness.clone(), premise.clone());
        assert!(
            check_certificate(&context, &invalid.conclusion, &[premise], &[], &invalid).is_err()
        );
    }
    for conclusion in [
        Proposition::LessThan(value(4, carrier), divisor),
        Proposition::LessThan(witness.target.clone(), value(4, carrier)),
        Proposition::LessThan(witness.target.clone(), literal(0, carrier)),
    ] {
        let invalid = ProofNode {
            conclusion,
            ..valid.clone()
        };
        assert!(
            check_certificate(
                &context,
                &invalid.conclusion,
                std::slice::from_ref(&positive),
                &[],
                &invalid
            )
            .is_err()
        );
    }
}

#[test]
fn unsigned_runtime_remainder_rejects_definition_and_carrier_drift() {
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = context(carrier);
    let divisor = value(2, carrier);
    let target = value(3, carrier);
    let expression =
        ScalarTerm::exact_integer_remainder(carrier, value(1, carrier), divisor.clone()).unwrap();
    let axioms = vec![
        Proposition::Equal(value(4, carrier), literal(9, carrier)),
        Proposition::Equal(target.clone(), expression.clone()),
    ];
    let valid = IntegerAffineWitness {
        root: divisor,
        target,
        definition_axioms: vec![1],
        literal_axioms: vec![None],
    };
    check_integer_affine_witness(&context, &axioms, &valid).unwrap();
    for (definitions, landings) in [
        (vec![], vec![]),
        (vec![0], vec![None]),
        (vec![2], vec![None]),
        (vec![1], vec![]),
        (vec![1], vec![Some(0)]),
        (vec![1, 0], vec![None, None]),
        (vec![0, 1], vec![None, None]),
        (vec![1, 1], vec![None, None]),
    ] {
        let invalid = IntegerAffineWitness {
            definition_axioms: definitions,
            literal_axioms: landings,
            ..valid.clone()
        };
        assert!(check_integer_affine_witness(&context, &axioms, &invalid).is_err());
    }
    for replacement in [
        Proposition::Equal(expression, valid.target.clone()),
        Proposition::Equal(
            valid.target.clone(),
            ScalarTerm::exact_integer_remainder(carrier, value(1, carrier), value(4, carrier))
                .unwrap(),
        ),
        Proposition::Equal(
            value(4, carrier),
            ScalarTerm::exact_integer_remainder(carrier, value(1, carrier), valid.root.clone())
                .unwrap(),
        ),
    ] {
        let changed = [axioms[0].clone(), replacement];
        assert!(check_integer_affine_witness(&context, &changed, &valid).is_err());
    }
    for other_carrier in [
        IntegerType::new(IntegerSign::Signed, 64).unwrap(),
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
    ] {
        let invalid = IntegerAffineWitness {
            root: value(2, other_carrier),
            ..valid.clone()
        };
        assert!(check_integer_affine_witness(&context, &axioms, &invalid).is_err());
    }
}

#[test]
fn unsigned_runtime_remainder_does_not_admit_signed_or_undeclared_operands() {
    let carrier = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let witness = IntegerAffineWitness {
        root: value(2, carrier),
        target: ScalarTerm::wrapping_integer_remainder(
            carrier,
            value(1, carrier),
            value(2, carrier),
        )
        .unwrap(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    assert!(check_integer_affine_witness(&context(carrier), &[], &witness).is_err());
    let carrier = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let witness = IntegerAffineWitness {
        root: value(2, carrier),
        target: ScalarTerm::exact_integer_remainder(carrier, value(99, carrier), value(2, carrier))
            .unwrap(),
        ..witness
    };
    assert!(check_integer_affine_witness(&context(carrier), &[], &witness).is_err());
}
