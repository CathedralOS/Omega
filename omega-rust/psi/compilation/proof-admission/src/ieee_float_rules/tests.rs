use super::*;
use crate::{
    AcceptedProofRule, MathematicalCoreDecision, PrimitiveJudgment, ProofNode, ProofRule,
    accept_certificate, decide_primitive,
};
use semantic_vocabulary::{
    ContentPlaceVersion, PlaceId, PropositionContext, StructuralPlaceKind, ValueId,
};

#[path = "model.rs"]
mod model;

fn relation(
    kind: IeeeFloatComparisonKind,
    format: IeeeFloatFormat,
    mut left: ScalarTerm,
    mut right: ScalarTerm,
) -> Proposition {
    if kind.is_symmetric() && left > right {
        std::mem::swap(&mut left, &mut right);
    }
    Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    }
}

fn literal(value: model::Value, format: model::Format) -> ScalarTerm {
    ScalarTerm::IeeeFloatLiteral(match format {
        model::Format::F32 => IeeeFloatValue::Binary32(u32::try_from(value.bits(format)).unwrap()),
        model::Format::F64 => IeeeFloatValue::Binary64(value.bits(format)),
    })
}

fn format(value: model::Format) -> IeeeFloatFormat {
    match value {
        model::Format::F32 => IeeeFloatFormat::Binary32,
        model::Format::F64 => IeeeFloatFormat::Binary64,
    }
}

fn order_kind(value: model::Ordered) -> IeeeFloatComparisonKind {
    match value {
        model::Ordered::Less => IeeeFloatComparisonKind::LessThan,
        model::Ordered::LessOrEqual => IeeeFloatComparisonKind::LessOrEqual,
    }
}

#[test]
fn closed_ieee_judgments_match_every_pair_of_the_small_model() {
    let model = model::Model::new();
    let context = PropositionContext::default();
    for embedding in [model::Format::F32, model::Format::F64] {
        for (a, left) in model.values.iter().enumerate() {
            for (b, right) in model.values.iter().enumerate() {
                assert_eq!(usize::from(left.encoding), a);
                let expected = model.pairs[a][b];
                for (kind, truth) in [
                    (IeeeFloatComparisonKind::Equal, expected.equal),
                    (IeeeFloatComparisonKind::NotEqual, expected.not_equal),
                    (IeeeFloatComparisonKind::LessThan, expected.less),
                    (IeeeFloatComparisonKind::LessOrEqual, expected.less_or_equal),
                    (IeeeFloatComparisonKind::NotLessThan, !expected.less),
                    (
                        IeeeFloatComparisonKind::NotLessOrEqual,
                        !expected.less_or_equal,
                    ),
                ] {
                    let goal = relation(
                        kind,
                        format(embedding),
                        literal(*left, embedding),
                        literal(*right, embedding),
                    );
                    assert_eq!(
                        decide_primitive(
                            &context,
                            &goal,
                            PrimitiveJudgment::ClosedIeeeFloatRelation
                        )
                        .is_ok(),
                        truth,
                        "{embedding:?} {a} {b} {kind:?}"
                    );
                    let negative = Proposition::Implication {
                        premise: Box::new(goal.clone()),
                        conclusion: Box::new(Proposition::Falsehood),
                    };
                    assert_eq!(
                        decide_primitive(
                            &context,
                            &negative,
                            PrimitiveJudgment::ClosedIeeeFloatRelation
                        )
                        .is_ok(),
                        !truth
                    );
                    assert!(
                        decide_primitive(&context, &goal, PrimitiveJudgment::ClosedIntegerRelation)
                            .is_err()
                    );
                    assert!(
                        decide_primitive(&context, &goal, PrimitiveJudgment::ReflexiveEquality)
                            .is_err()
                    );
                }
                for (kind, truth) in [
                    (IeeeFloatComparisonKind::LessThan, expected.greater),
                    (
                        IeeeFloatComparisonKind::LessOrEqual,
                        expected.greater_or_equal,
                    ),
                ] {
                    let goal = relation(
                        kind,
                        format(embedding),
                        literal(*right, embedding),
                        literal(*left, embedding),
                    );
                    assert_eq!(
                        decide_primitive(
                            &context,
                            &goal,
                            PrimitiveJudgment::ClosedIeeeFloatRelation
                        )
                        .is_ok(),
                        truth
                    );
                }
            }
        }
    }
}

#[test]
fn ordered_chains_match_all_small_model_triples_in_both_formats() {
    let model = model::Model::new();
    for embedding in [model::Format::F32, model::Format::F64] {
        let mut triples = 0;
        for triple in model.triples(embedding) {
            triples += 1;
            let declared_format = format(triple.format);
            let [a, b, c] = triple.values.map(|value| literal(value, embedding));
            for chain in triple.chains() {
                if !chain.premises_hold.into_iter().all(|truth| truth) {
                    continue;
                }
                let premises = [
                    relation(
                        order_kind(chain.left),
                        declared_format,
                        a.clone(),
                        b.clone(),
                    ),
                    relation(
                        order_kind(chain.right),
                        declared_format,
                        b.clone(),
                        c.clone(),
                    ),
                ];
                let goal = relation(
                    order_kind(chain.conclusion),
                    declared_format,
                    a.clone(),
                    c.clone(),
                );
                assert!(
                    chain.conclusion_holds,
                    "independent model disproves ordered transitivity"
                );
                check_chain(&goal, &premises).unwrap();
                // Pair judgments above independently check every closed
                // premise. Keep the exhaustive triple loop to cheap composition;
                // actual certificate traversal is checked by the controls below.
            }
        }
        assert_eq!(triples, model::TRIPLES_PER_FORMAT);
    }
}

fn assume(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    }
}

fn certificate(goal: Proposition, premises: &[Proposition]) -> ProofNode {
    ProofNode {
        conclusion: goal,
        rule: ProofRule::IeeeFloatOrderChain(
            premises
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, premise)| assume(premise, index))
                .collect(),
        ),
    }
}

#[test]
fn ordered_certificates_keep_actual_premises_and_reject_changed_subjects() {
    let scalar = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
    let [a, middle, c, other] =
        [1, 2, 3, 4].map(|id| ScalarTerm::value(ValueId::new(id).unwrap(), scalar));
    let context =
        PropositionContext::from_value_types((1..=4).map(|id| (ValueId::new(id).unwrap(), scalar)))
            .unwrap();
    let le = |left, right| {
        relation(
            IeeeFloatComparisonKind::LessOrEqual,
            IeeeFloatFormat::Binary64,
            left,
            right,
        )
    };
    let premises = [le(a.clone(), middle.clone()), le(middle.clone(), c.clone())];
    let goal = le(a.clone(), c.clone());
    let proof = certificate(goal.clone(), &premises);
    let accepted = accept_certificate(&context, &goal, &premises, &[], &proof).unwrap();
    assert_eq!(
        accepted
            .assumptions
            .iter()
            .map(|row| row.index)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!(
        accepted
            .rules
            .contains(&AcceptedProofRule::IeeeFloatOrderChain)
    );
    assert!(AcceptedProofRule::IeeeFloatOrderChain.is_provisional());
    assert!(matches!(
        accepted.mathematical_core,
        MathematicalCoreDecision::Refused(_)
    ));
    let mut with_axiom = proof.clone();
    let ProofRule::IeeeFloatOrderChain(children) = &mut with_axiom.rule else {
        unreachable!()
    };
    children[1].rule = ProofRule::SemanticAxiom { index: 0 };
    let accepted = accept_certificate(
        &context,
        &goal,
        &premises[..1],
        &[premises[1].clone(), Proposition::Truth],
        &with_axiom,
    )
    .unwrap();
    assert_eq!(
        accepted
            .assumptions
            .iter()
            .map(|row| row.index)
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        accepted
            .semantic_axioms
            .iter()
            .map(|row| row.index)
            .collect::<Vec<_>>(),
        vec![0]
    );
    for changed in 0..2 {
        let mut forged = proof.clone();
        let ProofRule::IeeeFloatOrderChain(children) = &mut forged.rule else {
            unreachable!()
        };
        children[changed].rule = ProofRule::Assumption { index: 99 };
        assert!(
            accept_certificate(&context, &goal, &premises, &[], &forged).is_err(),
            "every child must be independently checked"
        );
    }
    let disconnected = [premises[0].clone(), le(other, c.clone())];
    assert!(
        accept_certificate(
            &context,
            &goal,
            &disconnected,
            &[],
            &certificate(goal.clone(), &disconnected)
        )
        .is_err()
    );
    for missing in [Vec::new(), vec![premises[0].clone()]] {
        assert!(
            accept_certificate(
                &context,
                &goal,
                &premises,
                &[],
                &certificate(goal.clone(), &missing)
            )
            .is_err()
        );
    }
    let self_order = le(a.clone(), a.clone());
    assert!(
        accept_certificate(
            &context,
            &self_order,
            &[],
            &[],
            &certificate(self_order.clone(), &[])
        )
        .is_err()
    );
    assert!(
        decide_primitive(
            &context,
            &self_order,
            PrimitiveJudgment::ClosedIeeeFloatRelation
        )
        .is_err()
    );
    let strict = relation(
        IeeeFloatComparisonKind::LessThan,
        IeeeFloatFormat::Binary64,
        a,
        c,
    );
    assert!(
        accept_certificate(
            &context,
            &strict,
            &premises,
            &[],
            &certificate(strict.clone(), &premises)
        )
        .is_err()
    );
}

#[test]
fn unordered_formats_and_storage_versions_cannot_supply_order_edges() {
    let f = IeeeFloatFormat::Binary64;
    let zero = ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0));
    let nan = ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042));
    let complement = relation(
        IeeeFloatComparisonKind::NotLessThan,
        f,
        nan.clone(),
        zero.clone(),
    );
    decide_primitive(
        &PropositionContext::default(),
        &complement,
        PrimitiveJudgment::ClosedIeeeFloatRelation,
    )
    .unwrap();
    let false_order = relation(IeeeFloatComparisonKind::LessOrEqual, f, nan, zero.clone());
    assert!(check_chain(&false_order, [&complement]).is_err());
    let mixed = relation(
        IeeeFloatComparisonKind::LessOrEqual,
        f,
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0)),
        zero.clone(),
    );
    assert!(
        decide_primitive(
            &PropositionContext::default(),
            &mixed,
            PrimitiveJudgment::ClosedIeeeFloatRelation
        )
        .is_err()
    );
    let root = PlaceId::new(1).unwrap();
    let current = ScalarTerm::ieee_float_field_path(root, Vec::new(), f);
    let entry = ScalarTerm::IeeeFloatField {
        version: ContentPlaceVersion::Entry,
        root,
        path: Vec::new(),
        format: f,
    };
    let context = PropositionContext::from_value_types_and_places(
        [],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .unwrap();
    let goal = relation(
        IeeeFloatComparisonKind::LessOrEqual,
        f,
        zero.clone(),
        zero.clone(),
    );
    let premises = [
        relation(
            IeeeFloatComparisonKind::LessOrEqual,
            f,
            zero.clone(),
            current,
        ),
        relation(IeeeFloatComparisonKind::LessOrEqual, f, entry, zero),
    ];
    assert!(
        accept_certificate(
            &context,
            &goal,
            &premises,
            &[],
            &certificate(goal.clone(), &premises)
        )
        .is_err()
    );
}
