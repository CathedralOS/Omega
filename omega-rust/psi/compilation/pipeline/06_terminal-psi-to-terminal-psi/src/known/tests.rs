use super::*;
use terminal_check_tables::values::ValueOrigin;

fn node(conclusion: Proposition, rule: ProofRule) -> ProofNode {
    ProofNode { conclusion, rule }
}

fn eliminate(conclusion: Proposition, parent: ProofNode, conjunct: usize) -> ProofNode {
    node(
        conclusion,
        ProofRule::ConjunctionElimination {
            conjunction: Box::new(parent),
            conjunct,
        },
    )
}

fn check_all(
    known: &Known,
    context: &PropositionContext,
    assumptions: &[Proposition],
    axioms: &[Proposition],
) {
    for fact in &known.facts {
        assert!(!matches!(fact.origin, Origin::PendingConjunct));
        proof_admission::accept_certificate(
            context,
            &fact.proposition,
            assumptions,
            axioms,
            &fact.proof(&known.facts),
        )
        .unwrap();
    }
}

#[test]
fn nested_conjunctions_keep_postorder_and_exact_positional_evidence() {
    let first = Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::Boolean(true));
    let second = Proposition::Equal(ScalarTerm::Boolean(false), ScalarTerm::Boolean(false));
    let nested = Proposition::Conjunction(vec![second.clone(), first.clone()]);
    let last = Proposition::Truth;
    let root = Proposition::Conjunction(vec![first.clone(), nested.clone(), last.clone()]);
    let context = PropositionContext::default();
    let known = Known::new(
        &context,
        std::slice::from_ref(&root),
        std::slice::from_ref(&root),
        &IdTable::default(),
    );
    assert_eq!(
        known
            .facts
            .iter()
            .take(6)
            .map(|fact| fact.proposition.clone())
            .collect::<Vec<_>>(),
        [
            first.clone(),
            second,
            first.clone(),
            nested.clone(),
            last,
            root.clone()
        ]
    );
    assert_eq!(
        known
            .facts
            .iter()
            .map(|fact| fact.axiom)
            .collect::<Vec<_>>(),
        [
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(0)
        ]
    );
    let expected = eliminate(
        first.clone(),
        eliminate(
            nested,
            node(root.clone(), ProofRule::Assumption { index: 0 }),
            1,
        ),
        1,
    );
    assert_eq!(known.facts[2].proof(&known.facts), expected);
    check_all(
        &known,
        &context,
        std::slice::from_ref(&root),
        std::slice::from_ref(&root),
    );
    let mut wrong = expected;
    let ProofRule::ConjunctionElimination { conjunct, .. } = &mut wrong.rule else {
        unreachable!()
    };
    *conjunct = 0;
    assert!(
        proof_admission::accept_certificate(
            &context,
            &first,
            std::slice::from_ref(&root),
            std::slice::from_ref(&root),
            &wrong
        )
        .is_err()
    );
}

#[test]
fn an_empty_conjunction_keeps_its_origin_and_is_still_refused_by_the_kernel() {
    let malformed = Proposition::Conjunction(Vec::new());
    let context = PropositionContext::default();
    let known = Known::new(
        &context,
        std::slice::from_ref(&malformed),
        &[],
        &IdTable::default(),
    );
    assert_eq!(known.facts.len(), 1);
    let expected = node(malformed.clone(), ProofRule::Assumption { index: 0 });
    assert_eq!(known.facts[0].proof(&known.facts), expected);
    assert!(matches!(
        proof_admission::accept_certificate(
            &context,
            &malformed,
            std::slice::from_ref(&malformed),
            &[],
            &expected
        ),
        Err(proof_admission::ProofError::MalformedProposition(
            semantic_vocabulary::PropositionError::NonCanonicalConjunctionArity(0)
        ))
    ));
}

#[test]
fn repeated_equations_keep_first_definition_and_whole_axiom_identity() {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let value = values.push(ValueRow {
        scalar_type,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let equation = Proposition::Equal(
        ScalarTerm::value(value, scalar_type),
        ScalarTerm::integer(integer, IntegerValue::Unsigned(17)).unwrap(),
    );
    let pair = Proposition::Conjunction(vec![equation.clone(), equation.clone()]);
    let assumptions = [pair.clone(), equation.clone()];
    let axioms = [equation.clone()];
    let context = PropositionContext::from_value_types([(value, scalar_type)]).unwrap();
    let known = Known::new(&context, &assumptions, &axioms, &values);
    assert_eq!(known.equations, [0, 1, 3, 4]);
    assert_eq!(known.definitions.get(value), Some(&Some(0)));
    assert_eq!(known.facts[4].axiom, Some(0));
    assert!(known.facts[..4].iter().all(|fact| fact.axiom.is_none()));
    let expected = eliminate(
        equation.clone(),
        node(pair, ProofRule::Assumption { index: 0 }),
        0,
    );
    assert_eq!(
        known.facts[known.equations[0]].proof(&known.facts),
        expected
    );
    check_all(&known, &context, &assumptions, &axioms);
}

#[test]
fn integer_equality_lifts_keep_the_original_right_reflexive_child_in_both_orientations() {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let ids: Vec<_> = (0..2)
        .map(|position| {
            values.push(ValueRow {
                scalar_type,
                origin: ValueOrigin::MachineParameter { position },
                rigid: None,
            })
        })
        .collect();
    // Expressions on both sides need the embedding view without supplying a
    // value definition, in either orientation of the original relation.
    let expression = |id| ScalarTerm::ExactIntegerAdd {
        scalar_type: integer,
        left: Box::new(ScalarTerm::value(id, scalar_type)),
        right: Box::new(ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap()),
    };
    let left = expression(ids[0]);
    let right = expression(ids[1]);
    let context =
        PropositionContext::from_value_types(ids.iter().map(|&id| (id, scalar_type))).unwrap();
    let lifted = Proposition::IntegerMathEqual(
        IntegerMathTerm::embed(left.clone()).unwrap(),
        IntegerMathTerm::embed(right.clone()).unwrap(),
    );
    for (left, right) in [(left.clone(), right.clone()), (right, left)] {
        let equation = Proposition::Equal(left, right.clone());
        let known = Known::new(&context, std::slice::from_ref(&equation), &[], &values);
        assert!(
            known.equations.is_empty(),
            "a relation between expressions is not a value definition"
        );
        assert_eq!(known.facts.len(), 2);
        let expected = node(
            lifted.clone(),
            ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(node(
                    equation.clone(),
                    ProofRule::Assumption { index: 0 },
                )),
                middle_equals_right: Box::new(node(
                    Proposition::Equal(right.clone(), right),
                    ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                )),
            },
        );
        assert_eq!(known.facts[1].proof(&known.facts), expected);
        check_all(&known, &context, &[equation], &[]);
    }
}

#[test]
fn transported_branch_literals_capture_only_equations_available_before_the_consequence() {
    let mut values = IdTable::default();
    let ids: Vec<_> = (0..3)
        .map(|position| {
            values.push(ValueRow {
                scalar_type: ScalarType::Boolean,
                origin: ValueOrigin::MachineParameter { position },
                rigid: None,
            })
        })
        .collect();
    let term = |position| ScalarTerm::value(ids[position], ScalarType::Boolean);
    let yes = |position| Proposition::Equal(term(position), ScalarTerm::Boolean(true));
    let compare = |position| ScalarTerm::BooleanEqual {
        left: Box::new(term(position)),
        right: Box::new(ScalarTerm::Boolean(true)),
    };
    let axioms = [
        Proposition::Equal(term(1), compare(0)),
        Proposition::Equal(term(2), compare(1)),
    ];
    let assumptions = [yes(2)];
    let context =
        PropositionContext::from_value_types(ids.iter().map(|&id| (id, ScalarType::Boolean)))
            .unwrap();
    let known = Known::new(&context, &assumptions, &axioms, &values);
    let index = known
        .facts
        .iter()
        .position(|fact| fact.proposition == yes(0))
        .unwrap();
    assert_eq!(
        known.facts[known.equations[known.definitions.get(ids[0]).unwrap().unwrap()]].proposition,
        yes(0)
    );
    let Origin::Transport {
        premise,
        equalities,
    } = &known.facts[index].origin
    else {
        panic!("the branch consequence retains its exact transport")
    };
    assert_eq!(*premise, 0);
    assert_eq!(equalities, &[1, 2]);
    for (index, fact) in known.facts.iter().enumerate() {
        if let Origin::Transport { equalities, .. } = &fact.origin {
            assert!(equalities.iter().all(|&input| input < index));
        }
    }
    check_all(&known, &context, &assumptions, &axioms);
    let false_branch = [Proposition::Equal(term(2), ScalarTerm::Boolean(false))];
    assert!(
        proof_admission::accept_certificate(
            &context,
            &yes(0),
            &false_branch,
            &axioms,
            &known.facts[index].proof(&known.facts)
        )
        .is_err()
    );
}

#[test]
fn known_definitions_preserve_offset_value_identities() {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let first = ValueId::new(1_000_000).unwrap();
    let second = ValueId::new(1_000_001).unwrap();
    let values = IdTable::from_rows([first, second].into_iter().enumerate().map(
        |(position, id)| {
            (
                id,
                ValueRow {
                    scalar_type,
                    origin: ValueOrigin::MachineParameter {
                        position: position as u32,
                    },
                    rigid: None,
                },
            )
        },
    ))
    .unwrap();
    let equation = Proposition::Equal(
        ScalarTerm::value(second, scalar_type),
        ScalarTerm::integer(integer, IntegerValue::Unsigned(17)).unwrap(),
    );
    let context =
        PropositionContext::from_value_types([(first, scalar_type), (second, scalar_type)])
            .unwrap();
    let known = Known::new(&context, std::slice::from_ref(&equation), &[], &values);
    assert_eq!(
        known.definitions.keys().collect::<Vec<_>>(),
        [first, second]
    );
    assert_eq!(known.definitions.get(first), Some(&None));
    assert_eq!(known.definitions.get(second), Some(&Some(0)));
    assert_eq!(known.definitions.get(ValueId::new(1).unwrap()), None);
    check_all(&known, &context, std::slice::from_ref(&equation), &[]);
}
