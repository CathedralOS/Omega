use crate::{check_denotation_certificate, produce_denotation_certificates};
use semantic_vocabulary::{
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

fn context() -> PropositionContext {
    PropositionContext::from_value_types((1..=3).map(|index| {
        (
            ValueId::new(index).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        )
    }))
    .unwrap()
}

fn value(index: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(index).unwrap(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
    )
}

fn ordered(index: u64) -> Proposition {
    Proposition::ScalarIeeeFloatComparison {
        kind: IeeeFloatComparisonKind::Equal,
        format: IeeeFloatFormat::Binary32,
        left: value(index),
        right: value(index),
    }
}

fn bounds(index: u64) -> Proposition {
    Proposition::Conjunction(vec![
        Proposition::ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind::LessOrEqual,
            format: IeeeFloatFormat::Binary32,
            left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0xff7f_ffff)),
            right: value(index),
        },
        Proposition::ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind::LessOrEqual,
            format: IeeeFloatFormat::Binary32,
            left: value(index),
            right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x7f7f_ffff)),
        },
    ])
}

fn implies(premise: Proposition, conclusion: Proposition) -> Proposition {
    Proposition::Implication {
        premise: Box::new(premise),
        conclusion: Box::new(conclusion),
    }
}

#[test]
fn conditional_range_requires_the_exact_ordered_payload() {
    let context = context();
    let goal = bounds(3);
    let axioms = [implies(ordered(1), goal.clone())];
    let requirements = [ordered(1)];
    let certificates = produce_denotation_certificates(&context, &goal, &requirements, &axioms);
    assert!(!certificates.is_empty());
    for certificate in &certificates {
        assert!(check_denotation_certificate(
            &context,
            &goal,
            &requirements,
            &axioms,
            certificate,
        ));
        assert!(!check_denotation_certificate(
            &context,
            &goal,
            &[],
            &axioms,
            certificate,
        ));
        assert!(!check_denotation_certificate(
            &context,
            &goal,
            &[ordered(2)],
            &axioms,
            certificate,
        ));
    }
    // IEEE x == x is not ordinary reflexive equality: NaN remains unordered.
    assert!(produce_denotation_certificates(&context, &goal, &[], &axioms).is_empty());
    assert!(produce_denotation_certificates(&context, &goal, &[ordered(2)], &axioms).is_empty());
}

#[test]
fn conditional_chains_cannot_manufacture_a_missing_seed() {
    let context = context();
    let goal = bounds(3);
    let axioms = [
        implies(ordered(1), ordered(2)),
        implies(ordered(2), ordered(1)),
        implies(ordered(2), goal.clone()),
    ];
    assert!(produce_denotation_certificates(&context, &goal, &[], &axioms).is_empty());
    let requirements = [ordered(1)];
    assert!(
        produce_denotation_certificates(&context, &goal, &requirements, &axioms)
            .iter()
            .any(|certificate| check_denotation_certificate(
                &context,
                &goal,
                &requirements,
                &axioms,
                certificate,
            ))
    );
}

#[test]
fn ordinary_predicate_proof_keeps_original_equations_and_contract_citations() {
    let context = context();
    let goal = bounds(1);
    let axioms = [
        Proposition::Equal(value(1), value(3)),
        implies(ordered(2), bounds(3)),
        ordered(2),
    ];
    let proof = crate::produce_predicate_proof(&context, &goal, &[], &axioms)
        .expect("conditional range crosses an exact result alias");
    crate::check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    for changed in [
        vec![Proposition::Truth, axioms[1].clone(), axioms[2].clone()],
        vec![axioms[0].clone(), Proposition::Truth, axioms[2].clone()],
        vec![axioms[0].clone(), axioms[1].clone(), ordered(1)],
    ] {
        assert!(crate::check_certificate(&context, &goal, &[], &changed, &proof).is_err());
        assert!(crate::produce_predicate_proof(&context, &goal, &[], &changed).is_none());
    }
}

#[test]
fn conditional_search_keeps_the_enclosing_step_and_depth_limits() {
    let goal = ordered(2);
    let requirements = [ordered(1)];
    let axioms = [implies(ordered(1), goal.clone())];
    let mut remaining = 1;
    assert!(super::prove(&goal, &requirements, &axioms, &mut remaining, 0).is_none());
    assert_eq!(remaining, 0);
    let mut remaining = super::super::MAXIMUM_SEARCH_STEPS;
    assert!(
        super::prove(
            &goal,
            &requirements,
            &axioms,
            &mut remaining,
            super::super::MAXIMUM_PROOF_DEPTH,
        )
        .is_none()
    );
}

#[test]
fn definition_rosters_keep_conditional_contract_citations_and_input_limits() {
    let context = PropositionContext::from_value_types((1..=1503).map(|index| {
        (
            ValueId::new(index).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        )
    }))
    .unwrap();
    let definition = |index| {
        Proposition::Equal(
            value(index),
            ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0)),
        )
    };
    let mut axioms = (4..354).map(definition).collect::<Vec<_>>();
    let contract_index = axioms.len();
    axioms.push(implies(Proposition::Truth, bounds(3)));
    let goal = bounds(3);
    let proof = crate::produce_predicate_proof(&context, &goal, &[], &axioms)
        .expect("unrelated value definitions do not hide a normal-return contract");
    crate::check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    let mut changed = axioms.clone();
    changed[contract_index] = implies(ordered(1), bounds(3));
    assert!(crate::check_certificate(&context, &goal, &[], &changed, &proof).is_err());
    assert!(crate::produce_predicate_proof(&context, &goal, &[], &changed).is_none());
    changed[contract_index] = implies(Proposition::Truth, bounds(2));
    assert!(crate::check_certificate(&context, &goal, &[], &changed, &proof).is_err());
    assert!(crate::produce_predicate_proof(&context, &goal, &[], &changed).is_none());
    axioms.extend((354..=1503).map(definition));
    assert!(crate::produce_predicate_proof(&context, &goal, &[], &axioms).is_none());
}

#[test]
fn a_conjunctive_consequence_composes_through_a_later_conditional_contract() {
    let context = context();
    let requirements = [ordered(1)];
    let axioms = [
        implies(ordered(1), bounds(2)),
        implies(bounds(2), ordered(3)),
        implies(
            Proposition::Conjunction(vec![bounds(2), ordered(3)]),
            bounds(1),
        ),
    ];
    let goal = bounds(1);
    let proof = crate::produce_predicate_proof(&context, &goal, &requirements, &axioms)
        .expect("later contracts consume the proven range of earlier results");
    crate::check_certificate(&context, &goal, &requirements, &axioms, &proof).unwrap();
    assert!(crate::check_certificate(&context, &goal, &[], &axioms, &proof).is_err());
    assert!(crate::produce_predicate_proof(&context, &goal, &[], &axioms).is_none());
}

#[test]
fn restoring_repeated_citations_bounds_cloned_definition_children() {
    let context = context();
    let original = bounds(1);
    let equation = Proposition::Equal(value(1), value(3));
    let axioms = [original, equation.clone()];
    let equations = vec![
        crate::ProofNode {
            conclusion: equation,
            rule: crate::ProofRule::SemanticAxiom { index: 1 },
        };
        1000
    ];
    let child = crate::ProofNode {
        conclusion: bounds(3),
        rule: crate::ProofRule::SemanticAxiom { index: 0 },
    };
    let mut proof = crate::ProofNode {
        conclusion: Proposition::Conjunction(vec![bounds(3); 5]),
        rule: crate::ProofRule::ConjunctionIntroduction(vec![child; 5]),
    };
    assert!(
        crate::retain_original_predicate_citations(&mut proof, &context, &[], &axioms, &equations,)
            .is_none()
    );
}
