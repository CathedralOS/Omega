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
