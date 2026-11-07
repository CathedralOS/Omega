use super::*;
use crate::mathematical_core::{Level, Sort};
use crate::proof_node;

fn content_algebra() -> ContentAlgebra {
    ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Byte".to_owned(),
    }
}

fn content_root() -> PlaceId {
    PlaceId::new(1).expect("place")
}

fn content_context(root: PlaceId) -> PropositionContext {
    PropositionContext::from_value_types_and_places(
        [],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .expect("content context")
}

fn content_term(root: PlaceId, field: &str) -> ContentTerm {
    ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(2).expect("domain"),
            projection_report_fingerprint: 3,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root,
            segments: vec![ContentPlaceSegment::Field(field.to_owned())],
        },
    }
}

fn conservation(algebra: &ContentAlgebra, left: &ContentTerm, right: &ContentTerm) -> Proposition {
    Proposition::ContentConservation(ContentConservation::new(
        algebra.clone(),
        left.clone(),
        right.clone(),
    ))
}

fn assert_content_carrier_vocabulary(denoted: &BoundedDenotation) {
    let parameter = &denoted.certificate.signature[0];
    assert_eq!(parameter.level_arity, 0);
    assert!(parameter.is_assumption());
    assert_eq!(
        denoted.arena.get(parameter.ty),
        Term::Sort(Sort::Type(Level::Constant(0)))
    );
    let family = &denoted.certificate.signature[1];
    assert_eq!(family.level_arity, 1);
    assert!(family.is_assumption());
    let Term::Pi { domain, codomain } = denoted.arena.get(family.ty) else {
        panic!("the canonical carrier is an outer type family");
    };
    assert_eq!(
        denoted.arena.get(domain),
        Term::Sort(Sort::Type(Level::Parameter(0)))
    );
    assert_eq!(
        denoted.arena.get(codomain),
        Term::Sort(Sort::Type(Level::Constant(0)))
    );
}

/// `ContentConservation` denotes `Id` over its algebra's carrier, so
/// its transitivity rule is a `J` composition the kernel re-decides
/// in every shared-endpoint orientation the bounded relation check
/// licenses — never an interned rule axiom.
#[test]
fn content_conservation_transitivity_is_a_j_composition() {
    let root = content_root();
    let algebra = content_algebra();
    let terms = |field: &str| content_term(root, field);
    let context = content_context(root);
    // Every shared-endpoint arrangement the checker's
    // `ContentConservation` arm accepts: direct chaining, right-right
    // and left-left sharing, and the cross-side middle — the `J`
    // composition wraps the needed premises in `sym`.
    for (first_pair, second_pair, goal_pair) in [
        (("a", "b"), ("b", "c"), ("a", "c")),
        (("a", "c"), ("b", "c"), ("a", "b")),
        (("a", "c"), ("a", "b"), ("b", "c")),
        (("b", "c"), ("a", "b"), ("a", "c")),
    ] {
        let first = conservation(&algebra, &terms(first_pair.0), &terms(first_pair.1));
        let second = conservation(&algebra, &terms(second_pair.0), &terms(second_pair.1));
        let goal = conservation(&algebra, &terms(goal_pair.0), &terms(goal_pair.1));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(semantic_axiom_proof(first.clone(), 0)),
                middle_equals_right: Box::new(semantic_axiom_proof(second.clone(), 1)),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            &[],
            &[first, second],
            &proof,
            &mut budget(),
        )
        .unwrap_or_else(|error| {
            panic!("content transitivity {goal_pair:?} verifies as J: {error}")
        });
        // `{P, C, a', b', c'}` — the parameter, carrier family and one element
        // constant per distinct content term; the rule cites no
        // axiom of its own.
        assert_content_carrier_vocabulary(&denoted);
        assert_eq!(denoted.certificate.signature.len(), 5);
        assert!(
            denoted
                .certificate
                .signature
                .iter()
                .all(|declaration| declaration.is_assumption()),
        );
        assert!(
            matches!(
                denoted.arena.get(denoted.certificate.term),
                Term::IdElim { .. }
            ),
            "content transitivity {goal_pair:?} composes by J",
        );
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 3, 4]),
        );
        assert_eq!(
            denoted.rules,
            vec![
                AcceptedProofRule::SemanticAxiom,
                AcceptedProofRule::EqualityTransitivity
            ],
        );
        assert_eq!(denoted.semantic_axioms.len(), 2);
    }
}

/// A reflexive content conservation denotes `refl` on the algebra
/// carrier — kernel-proved, not a named decision — over `Separate`
/// endpoints as well as bare projections.
#[test]
fn reflexive_content_conservation_is_refl_in_the_kernel() {
    let root = content_root();
    let algebra = content_algebra();
    let context = content_context(root);
    let separate = ContentTerm::separate([content_term(root, "a"), content_term(root, "b")])
        .expect("separation");
    let proposition = conservation(&algebra, &separate, &separate);
    let proof = proof_node!(proposition.clone() => ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality));
    let denoted =
        verify_bounded_certificate(&context, &proposition, &[], &[], &proof, &mut budget())
            .expect("reflexive conservation verifies as refl");
    // `{P, C, s'}` — the parameter, carrier family and one endpoint;
    // the evidence is `refl (C P) s'` with no decision assumption.
    assert_content_carrier_vocabulary(&denoted);
    assert_eq!(denoted.certificate.signature.len(), 3);
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::Refl { .. }
    ));
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2]),
    );
}

/// Content transitivity still rejects at the shared checker's own
/// points: different algebras, no shared endpoint, or a conclusion
/// that is not the composed equation.
#[test]
fn content_conservation_transitivity_rejects_at_the_shared_check() {
    let root = content_root();
    let algebra = content_algebra();
    let terms = |field: &str| content_term(root, field);
    let context = content_context(root);
    let transitivity = |first: Proposition, second: Proposition, goal: Proposition| ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(semantic_axiom_proof(first, 0)),
            middle_equals_right: Box::new(semantic_axiom_proof(second, 1)),
        },
    };

    // A shared endpoint across different algebras is an algebra
    // mismatch — the denotation would put the equations over
    // different carriers anyway.
    let other = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Gram".to_owned(),
    };
    let first = conservation(&algebra, &terms("a"), &terms("b"));
    let second = conservation(&other, &terms("b"), &terms("c"));
    let goal = conservation(&algebra, &terms("a"), &terms("c"));
    let proof = transitivity(first.clone(), second.clone(), goal.clone());
    assert!(matches!(
        denote_bounded_certificate(&context, &goal, &[], &[first, second], &proof),
        Err(BoundedDenotationError::Certificate(
            ProofError::EqualityAlgebraMismatch
        )),
    ));

    // No shared endpoint at all.
    let first = conservation(&algebra, &terms("a"), &terms("b"));
    let second = conservation(&algebra, &terms("c"), &terms("d"));
    let proof = transitivity(first.clone(), second.clone(), goal.clone());
    assert!(matches!(
        denote_bounded_certificate(&context, &goal, &[], &[first, second], &proof),
        Err(BoundedDenotationError::Certificate(
            ProofError::EqualityMiddleMismatch
        )),
    ));

    // A shared endpoint but a conclusion that is not the composed
    // equation.
    let first = conservation(&algebra, &terms("a"), &terms("b"));
    let second = conservation(&algebra, &terms("b"), &terms("c"));
    let proof = transitivity(first.clone(), second.clone(), first.clone());
    assert!(matches!(
        denote_bounded_certificate(&context, &first.clone(), &[], &[first, second], &proof),
        Err(BoundedDenotationError::Certificate(
            ProofError::EqualityConclusionMismatch
        )),
    ));
}

/// Unchanged content evidence is already the requested proof. Its
/// original context remains in the closure even when transport is empty.
#[test]
fn unchanged_content_transport_reuses_evidence_and_retains_its_context() {
    let root = content_root();
    let algebra = content_algebra();
    let premise = conservation(&algebra, &content_term(root, "a"), &content_term(root, "b"));
    let (x_id, x) = value(1);
    let (z_id, z) = value(3);
    let equation = Proposition::Equal(x, z);
    let context = PropositionContext::from_value_types_and_places(
        [(x_id, unsigned64_type()), (z_id, unsigned64_type())],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .expect("context");
    let proof = ProofNode {
        conclusion: premise.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(semantic_axiom_proof(premise.clone(), 0)),
            equalities: vec![semantic_axiom_proof(equation.clone(), 1)],
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &premise,
        &[],
        &[premise.clone(), equation],
        &proof,
        &mut budget(),
    )
    .expect("the original content evidence verifies");
    // P, C, a', b', Int, m_x and m_z remain; no new rule axiom.
    assert_content_carrier_vocabulary(&denoted);
    assert_eq!(denoted.certificate.signature.len(), 7);
    assert_eq!(
        denoted.arena.get(denoted.certificate.term),
        Term::Variable(1)
    );
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        (0..7).collect()
    );
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.context[0]),
            Term::Id { .. }
        ),
        "the first premise is the content identity",
    );
    assert_eq!(
        denoted.rules,
        vec![
            AcceptedProofRule::SemanticAxiom,
            AcceptedProofRule::ValueEqualityTransport
        ],
    );
}
