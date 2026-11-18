use super::*;
use semantic_vocabulary::{IntegerSign, OperationId};
use terminal_check_tables::values::{ValueOrigin, ValueRow};

mod exact_fact_index {
    use super::*;

    struct CollidingHasher;

    impl std::hash::Hasher for CollidingHasher {
        fn finish(&self) -> u64 {
            0
        }

        fn write(&mut self, _: &[u8]) {}
    }

    impl BuildHasher for CollidingHasher {
        type Hasher = Self;

        fn build_hasher(&self) -> Self {
            Self
        }
    }

    struct NoTermHash;

    impl BuildHasher for NoTermHash {
        type Hasher = CollidingHasher;

        fn build_hasher(&self) -> Self::Hasher {
            panic!("recursive terms must keep the exact fallback path")
        }
    }

    fn integer() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    }

    fn literal(n: u128) -> ScalarTerm {
        ScalarTerm::integer(integer(), IntegerValue::Unsigned(n)).unwrap()
    }

    fn value(id: u64) -> ScalarTerm {
        ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Integer(integer()))
    }

    fn inputs() -> IdTable<ValueId, ValueRow> {
        IdTable::from_rows((0..2).map(|position| {
            (
                ValueId::new(u64::from(position) + 1).unwrap(),
                ValueRow {
                    scalar_type: ScalarType::Integer(integer()),
                    origin: ValueOrigin::MachineParameter { position },
                    rigid: None,
                },
            )
        }))
        .unwrap()
    }

    fn context(values: &IdTable<ValueId, ValueRow>) -> PropositionContext {
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap()
    }

    #[test]
    fn collisions_keep_exact_equality_first_ordinal_and_missing_goals() {
        let values = inputs();
        let context = context(&values);
        let first = Proposition::LessOrEqual(value(1), literal(7));
        let second = Proposition::LessOrEqual(value(2), literal(9));
        let missing = Proposition::LessOrEqual(value(1), literal(8));
        let assumptions = [
            first.clone(),
            second.clone(),
            first.clone(),
            Proposition::Falsehood,
        ];
        let axioms = [first.clone()];
        let known = crate::checking::known::Known::new(&context, &assumptions, &axioms, &values);
        let index = ExactFacts::with_hasher(&known.facts, CollidingHasher);
        let mut activity = ProofSearchActivity::default();
        assert_eq!(index.find(&known.facts, &first, &mut activity), Some(0));
        assert_eq!(index.find(&known.facts, &second, &mut activity), Some(1));
        assert_eq!(index.find(&known.facts, &missing, &mut activity), None);
        assert!(activity.exact_fact_work.miss_comparisons >= known.facts.len());
    }

    #[test]
    fn an_exact_goal_precedes_falsehood_and_keeps_its_earliest_proof() {
        let values = inputs();
        let context = context(&values);
        let goal = Proposition::LessOrEqual(value(1), literal(7));
        let assumptions = [Proposition::Falsehood, goal.clone(), goal.clone()];
        let axioms = [goal.clone()];
        let mut activity = ProofSearchActivity::default();
        let proof = Prover::new(&context, &assumptions, &axioms, &values, &mut activity)
            .prove(&goal)
            .unwrap();
        let expected = node(goal.clone(), ProofRule::Assumption { index: 1 });
        assert_eq!(format!("{proof:?}"), format!("{expected:?}"));
        let accepted =
            proof_admission::accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
                .unwrap();
        assert_eq!(
            accepted
                .assumptions
                .iter()
                .map(|used| used.index)
                .collect::<Vec<_>>(),
            [1]
        );
        assert!(accepted.semantic_axioms.is_empty());
    }

    #[test]
    fn falsehood_remains_available_but_no_answer_crosses_premise_contexts() {
        let values = inputs();
        let context = context(&values);
        let goal = Proposition::LessOrEqual(value(1), literal(7));
        let assumptions = [Proposition::Falsehood];
        let mut activity = ProofSearchActivity::default();
        let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
            .prove(&goal)
            .unwrap();
        proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof).unwrap();
        assert!(
            Prover::new(&context, &[], &[], &values, &mut activity)
                .prove(&goal)
                .is_err()
        );
    }

    #[test]
    fn transport_truth_keeps_priority_over_a_truth_fact() {
        let values = inputs();
        let context = context(&values);
        let goal = Proposition::Equal(value(1), literal(7));
        let assumptions = [goal.clone(), Proposition::Truth];
        let mut activity = ProofSearchActivity::default();
        let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
        let proof = prover.by_transport(&goal).unwrap();
        let ProofRule::ValueEqualityTransport { premise, .. } = &proof.rule else {
            panic!("the exact equality must be carried by its existing transport rule");
        };
        assert!(matches!(
            premise.rule,
            ProofRule::Primitive(PrimitiveJudgment::Truth)
        ));
        let accepted =
            proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof)
                .unwrap();
        assert_eq!(
            accepted
                .assumptions
                .iter()
                .map(|used| used.index)
                .collect::<Vec<_>>(),
            [0]
        );
    }

    #[test]
    fn recursive_terms_and_deep_logical_facts_use_exact_fallback_without_hashing() {
        let values = inputs();
        let context = context(&values);
        let mut logical = Proposition::Truth;
        for _ in 0..512 {
            logical = Proposition::Disjunction(vec![logical]);
        }
        let mathematical = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
                Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(2))),
            ),
            IntegerMathTerm::literal(IntegerValue::Unsigned(4)),
        );
        let scalar = Proposition::LessOrEqual(
            ScalarTerm::exact_integer_add(integer(), value(1), literal(1)).unwrap(),
            literal(9),
        );
        let assumptions = [logical, mathematical, scalar];
        let known = crate::checking::known::Known::new(&context, &assumptions, &[], &values);
        let index = ExactFacts::with_hasher(&known.facts, NoTermHash);
        let mut activity = ProofSearchActivity::default();
        for goal in &assumptions {
            assert!(!exact_fact_hash_eligible(goal));
            assert_eq!(
                index.find(&known.facts, goal, &mut activity),
                known
                    .facts
                    .iter()
                    .position(|fact| &fact.proposition == goal)
            );
        }
        assert_eq!(activity.exact_fact_work.fallback_lookups, 3);
    }
}

mod bound_fact_index {
    use super::*;
    use semantic_vocabulary::PlaceId;

    fn integer() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    }

    fn literal(n: u128) -> ScalarTerm {
        ScalarTerm::integer(integer(), IntegerValue::Unsigned(n)).unwrap()
    }

    fn value(id: ValueId) -> ScalarTerm {
        ScalarTerm::value(id, ScalarType::Integer(integer()))
    }

    fn inputs(first: u64, count: usize) -> IdTable<ValueId, ValueRow> {
        IdTable::from_rows((0..count).map(|position| {
            (
                ValueId::new(first + u64::try_from(position).unwrap()).unwrap(),
                ValueRow {
                    scalar_type: ScalarType::Integer(integer()),
                    origin: ValueOrigin::MachineParameter {
                        position: u32::try_from(position).unwrap(),
                    },
                    rigid: None,
                },
            )
        }))
        .unwrap()
    }

    fn context(values: &IdTable<ValueId, ValueRow>) -> PropositionContext {
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap()
    }

    #[test]
    fn ordinals_merge_partial_visits_once_in_the_original_order() {
        let values = inputs(31, 3);
        let subject = values.identity_at(0).unwrap();
        let other = values.identity_at(1).unwrap();
        let third = values.identity_at(2).unwrap();
        let opaque = ScalarTerm::ViewExtent {
            root: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            scalar_type: integer(),
        };
        let assumptions = [
            Proposition::LessOrEqual(value(other), literal(7)),
            Proposition::LessOrEqual(value(subject), literal(5)),
            Proposition::Equal(
                ScalarTerm::exact_integer_add(integer(), value(subject), value(subject)).unwrap(),
                value(third),
            ),
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::embed(value(subject)).unwrap(),
                IntegerMathTerm::literal(IntegerValue::Unsigned(9)),
            ),
            Proposition::Equal(value(subject), opaque.clone()),
            Proposition::Equal(value(other), opaque),
        ];
        let context = context(&values);
        let mut activity = ProofSearchActivity::default();
        let prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
        // This checks candidate coverage, not admission of the opaque terms.
        // Preparation also retains the embedding equalities of the three
        // scalar equalities. They are candidates with their own ordinals.
        assert_eq!(prover.facts.len(), 9);
        for (derived, original) in [(6, 2), (7, 4), (8, 5)] {
            let Proposition::Equal(left, right) = &assumptions[original] else {
                unreachable!();
            };
            assert_eq!(
                prover.facts[derived].proposition,
                sorted_math_equal(
                    IntegerMathTerm::embed(left.clone()).unwrap(),
                    IntegerMathTerm::embed(right.clone()).unwrap(),
                )
            );
        }
        assert_eq!(prover.bound_facts.fallback, [4, 5, 7, 8]);
        let candidates = |id| {
            let mut cursor = BoundFactCursor::default();
            std::iter::from_fn(|| prover.bound_facts.next(id, &mut cursor)).collect::<Vec<_>>()
        };
        assert_eq!(candidates(subject), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(candidates(other), [0, 4, 5, 7, 8]);
        assert_eq!(candidates(third), [2, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn scalar_compound_and_mathematical_bounds_keep_their_evidence() {
        let values = inputs(41, 1);
        let id = values.identity_at(0).unwrap();
        let subject = value(id);
        let context = context(&values);
        let math_subject = IntegerMathTerm::embed(subject.clone()).unwrap();
        let cases = [
            (
                Proposition::LessOrEqual(literal(3), subject.clone()),
                Direction::Lower,
                3,
            ),
            (
                Proposition::LessThan(subject.clone(), literal(10)),
                Direction::Upper,
                9,
            ),
            (
                Proposition::Equal(literal(7), subject.clone()),
                Direction::Upper,
                7,
            ),
            (
                Proposition::IntegerMathLessOrEqual(
                    math_subject.clone(),
                    IntegerMathTerm::literal(IntegerValue::Unsigned(9)),
                ),
                Direction::Upper,
                9,
            ),
            (
                sorted_math_equal(
                    math_subject,
                    IntegerMathTerm::literal(IntegerValue::Unsigned(7)),
                ),
                Direction::Upper,
                7,
            ),
            (
                Proposition::LessOrEqual(
                    ScalarTerm::exact_integer_add(integer(), subject, literal(2)).unwrap(),
                    literal(10),
                ),
                Direction::Upper,
                8,
            ),
        ];
        for (assumption, direction, expected) in cases {
            let assumptions = [assumption];
            let mut activity = ProofSearchActivity::default();
            let bound = Prover::new(&context, &assumptions, &[], &values, &mut activity)
                .bound(id, direction)
                .unwrap();
            assert_eq!(bound.literal, IntegerValue::Unsigned(expected));
            proof_admission::accept_certificate(
                &context,
                &bound.proof.conclusion,
                &assumptions,
                &[],
                &bound.proof,
            )
            .unwrap();
        }
    }

    #[test]
    fn tied_bounds_keep_the_first_certificate_and_original_premise_ordinal() {
        let values = inputs(11, 2);
        let subject = values.identity_at(0).unwrap();
        let other = values.identity_at(1).unwrap();
        let context = context(&values);
        let assumptions = [
            Proposition::LessOrEqual(value(other), literal(7)),
            Proposition::LessThan(value(subject), literal(10)),
            Proposition::LessOrEqual(value(subject), literal(9)),
        ];
        let mut activity = ProofSearchActivity::default();
        let bound = Prover::new(&context, &assumptions, &[], &values, &mut activity)
            .bound(subject, Direction::Upper)
            .unwrap();
        let expected = node(
            assumptions[2].clone(),
            ProofRule::IntegerOrderDiscreteness {
                relation: Box::new(node(
                    assumptions[1].clone(),
                    ProofRule::Assumption { index: 1 },
                )),
            },
        );
        assert_eq!(bound.proof, expected);
        assert_eq!(activity.literal_bound_fact_nodes, 1);
        proof_admission::accept_certificate(
            &context,
            &expected.conclusion,
            &assumptions,
            &[],
            &bound.proof,
        )
        .unwrap();

        // A strict comparison can improve a bound with the same written number.
        let raw_tie = [
            Proposition::LessOrEqual(value(subject), literal(10)),
            Proposition::LessThan(value(subject), literal(10)),
        ];
        let mut activity = ProofSearchActivity::default();
        let mut prover = Prover::new(&context, &raw_tie, &[], &values, &mut activity);
        let bound = prover.bound(subject, Direction::Upper).unwrap();
        assert_eq!(bound.literal, IntegerValue::Unsigned(9));
        assert_eq!(prover.activity.literal_bound_fact_nodes, 2);
        let goal = Proposition::LessOrEqual(value(subject), literal(9));
        let expected = node(
            goal.clone(),
            ProofRule::IntegerOrderDiscreteness {
                relation: Box::new(node(raw_tie[1].clone(), ProofRule::Assumption { index: 1 })),
            },
        );
        assert_eq!(bound.proof, expected);
        proof_admission::accept_certificate(&context, &goal, &raw_tie, &[], &bound.proof).unwrap();
        assert!(
            prover
                .prove(&Proposition::LessOrEqual(value(subject), literal(8)))
                .is_err()
        );
    }

    #[test]
    fn unrelated_bounds_still_supply_their_actual_contradiction() {
        let values = inputs(51, 2);
        let subject = values.identity_at(0).unwrap();
        let other = values.identity_at(1).unwrap();
        let context = context(&values);
        let assumptions = [
            Proposition::LessOrEqual(value(other), literal(3)),
            Proposition::LessOrEqual(literal(4), value(other)),
        ];
        let goal = Proposition::LessThan(value(subject), literal(0));
        let mut activity = ProofSearchActivity::default();
        let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
            .prove(&goal)
            .unwrap();
        let accepted =
            proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof)
                .unwrap();
        let mut cited: Vec<_> = accepted.assumptions.iter().map(|used| used.index).collect();
        cited.sort_unstable();
        assert_eq!(cited, [0, 1]);
        for removed in [0, 1] {
            let remaining = [assumptions[1 - removed].clone()];
            assert!(
                Prover::new(
                    &context,
                    &remaining,
                    &[],
                    &values,
                    &mut ProofSearchActivity::default(),
                )
                .prove(&goal)
                .is_err()
            );
        }
    }

    #[test]
    fn each_split_uses_its_own_complete_context() {
        let values = inputs(21, 2);
        let subject = values.identity_at(0).unwrap();
        let other = values.identity_at(1).unwrap();
        let context = context(&values);
        let goal = Proposition::LessOrEqual(value(subject), literal(5));
        for (second, succeeds) in [(subject, true), (other, false)] {
            let assumptions = [Proposition::Disjunction(vec![
                Proposition::LessOrEqual(value(subject), literal(3)),
                Proposition::LessOrEqual(value(second), literal(5)),
            ])];
            let mut activity = ProofSearchActivity::default();
            let result =
                Prover::new(&context, &assumptions, &[], &values, &mut activity).prove(&goal);
            assert_eq!(result.is_ok(), succeeds);
            if let Ok(proof) = result {
                proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof)
                    .unwrap();
            }
        }
    }

    #[test]
    fn a_bound_does_not_scan_unrelated_literal_premises() {
        for count in [8, 128] {
            let values = inputs(61, count + 1);
            let subject = values.identity_at(0).unwrap();
            let context = context(&values);
            let mut assumptions: Vec<_> = values
                .keys()
                .skip(1)
                .map(|id| Proposition::LessOrEqual(value(id), literal(100)))
                .collect();
            assumptions.push(Proposition::LessOrEqual(value(subject), literal(9)));
            let mut activity = ProofSearchActivity::default();
            let bound = Prover::new(&context, &assumptions, &[], &values, &mut activity)
                .bound(subject, Direction::Upper)
                .unwrap();
            assert_eq!(bound.literal, IntegerValue::Unsigned(9));
            proof_admission::accept_certificate(
                &context,
                &bound.proof.conclusion,
                &assumptions,
                &[],
                &bound.proof,
            )
            .unwrap();
            assert_eq!(activity.bound_value_searches, 1);
            assert!(activity.bound_fact_visits <= 4);
        }
    }
}

#[test]
fn failed_opposite_searches_leave_candidate_fact_proofs_unbuilt() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let value = values.push(ValueRow {
        scalar_type: scalar,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let condition = values.push(ValueRow {
        scalar_type: ScalarType::Boolean,
        origin: ValueOrigin::MachineParameter { position: 1 },
        rigid: None,
    });
    let value = ScalarTerm::value(value, scalar);
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(3)).unwrap();
    let candidates = [
        Proposition::LessOrEqual(value.clone(), literal.clone()),
        Proposition::Equal(value, literal),
        Proposition::Equal(
            ScalarTerm::value(condition, ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        ),
    ];
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let mut constructed = Vec::new();
    for candidate in candidates {
        let assumptions = [candidate];
        let mut activity = ProofSearchActivity::default();
        let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
        let before = prover.activity.refutation_fact_nodes;
        assert!(prover.prove(&Proposition::Falsehood).is_err());
        constructed.push(prover.activity.refutation_fact_nodes - before);
        assert_eq!(prover.remaining_case_branches, CASE_BRANCH_BUDGET);
    }
    // Check every refusal before comparing the candidate trees. Proofs
    // constructed inside the opposite search are separate work.
    assert_eq!(constructed, [0, 0, 0]);
}

#[test]
fn symbolic_ieee_bounds_exclude_their_exact_crash_complements() {
    use semantic_vocabulary::{IeeeFloatComparisonKind as Kind, IeeeFloatFormat, IeeeFloatValue};
    let mut activity = ProofSearchActivity::default();
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar = ScalarType::IeeeFloat(format);
        let mut values = IdTable::default();
        let id = values.push(ValueRow {
            scalar_type: scalar,
            origin: ValueOrigin::MachineParameter { position: 0 },
            rigid: None,
        });
        let value = ScalarTerm::value(id, scalar);
        let zero = ScalarTerm::IeeeFloatLiteral(match format {
            IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
            IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
        });
        let one = ScalarTerm::IeeeFloatLiteral(match format {
            IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0x3f80_0000),
            IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
        });
        let relation = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        };
        let assumptions = [
            relation(Kind::Equal, value.clone(), value.clone()),
            relation(Kind::LessOrEqual, zero.clone(), value.clone()),
            relation(Kind::LessThan, value.clone(), one.clone()),
        ];
        let failures = [
            relation(Kind::NotEqual, value.clone(), value.clone()),
            relation(Kind::LessThan, value.clone(), zero.clone()),
            relation(Kind::LessOrEqual, one, value.clone()),
        ];
        let context = PropositionContext::from_value_types([(id, scalar)]).unwrap();
        for (index, failure) in failures.iter().enumerate() {
            let goal = Proposition::Implication {
                premise: Box::new(failure.clone()),
                conclusion: Box::new(Proposition::Falsehood),
            };
            // Equality alone is the Eq/Ne premise; the bounds are separate
            // cases, since their positive order already excludes NaN.
            let selected = if index == 0 {
                &assumptions[..1]
            } else {
                &assumptions[..]
            };
            let proof = Prover::new(&context, selected, &[], &values, &mut activity)
                .prove(&goal)
                .unwrap();
            let accepted =
                proof_admission::accept_certificate(&context, &goal, selected, &[], &proof)
                    .unwrap();
            assert!(accepted.semantic_axioms.is_empty());
            assert!(accepted.assumptions.iter().any(|used| used.index == index));
            let mut missing = selected.to_vec();
            missing.remove(index);
            assert!(
                Prover::new(&context, &missing, &[], &values, &mut activity)
                    .prove(&goal)
                    .is_err()
            );
        }
        // An exact logical complement can refute its matching predicate,
        // but does not establish the reversed positive order for NaN.
        let unordered_lower = relation(Kind::NotLessThan, value.clone(), zero.clone());
        let goal = Proposition::Implication {
            premise: Box::new(relation(Kind::LessThan, value.clone(), zero.clone())),
            conclusion: Box::new(Proposition::Falsehood),
        };
        let proof = Prover::new(
            &context,
            std::slice::from_ref(&unordered_lower),
            &[],
            &values,
            &mut activity,
        )
        .prove(&goal)
        .unwrap();
        proof_admission::accept_certificate(
            &context,
            &goal,
            std::slice::from_ref(&unordered_lower),
            &[],
            &proof,
        )
        .unwrap();
        assert!(
            Prover::new(&context, &[unordered_lower], &[], &values, &mut activity)
                .prove(&relation(Kind::LessOrEqual, zero, value))
                .is_err()
        );
    }
}

#[test]
fn false_ieee_assumptions_use_checked_contradiction_with_the_actual_equation() {
    use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue};
    let mut activity = ProofSearchActivity::default();
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar = ScalarType::IeeeFloat(format);
        let mut values = IdTable::default();
        let value = values.push(ValueRow {
            scalar_type: scalar,
            origin: ValueOrigin::Operation(OperationId::new(1).unwrap()),
            rigid: None,
        });
        let value = ScalarTerm::value(value, scalar);
        let [zero, quarter, nan] = match format {
            IeeeFloatFormat::Binary32 => {
                [0, 0x3e80_0000, 0x7fc0_0042].map(IeeeFloatValue::Binary32)
            }
            IeeeFloatFormat::Binary64 => {
                [0, 0x3fd0_0000_0000_0000, 0x7ff8_0000_0000_0042].map(IeeeFloatValue::Binary64)
            }
        }
        .map(ScalarTerm::IeeeFloatLiteral);
        let comparison = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        };
        let unordered = comparison(
            IeeeFloatComparisonKind::NotEqual,
            value.clone(),
            value.clone(),
        );
        let invalid = Proposition::Disjunction(vec![
            unordered.clone(),
            comparison(IeeeFloatComparisonKind::LessThan, value.clone(), zero),
        ]);
        let excludes_invalid = Proposition::Implication {
            premise: Box::new(invalid),
            conclusion: Box::new(Proposition::Falsehood),
        };
        let context = PropositionContext::from_value_types(
            values.iter().map(|(id, row)| (id, row.scalar_type)),
        )
        .unwrap();
        for (literal, excludes) in [(quarter, true), (nan, false)] {
            let axioms = [Proposition::Equal(value.clone(), literal)];
            let result = Prover::new(&context, &[], &axioms, &values, &mut activity)
                .prove(&excludes_invalid);
            assert_eq!(result.is_ok(), excludes);
            if let Ok(proof) = result {
                let accepted = proof_admission::accept_certificate(
                    &context,
                    &excludes_invalid,
                    &[],
                    &axioms,
                    &proof,
                )
                .unwrap();
                assert!(accepted.assumptions.is_empty());
                assert_eq!(accepted.semantic_axioms.len(), 1);
                assert_eq!(accepted.semantic_axioms[0].index, 0);
            }
            let assumptions = [Proposition::Truth, unordered.clone()];
            let result = Prover::new(&context, &assumptions, &axioms, &values, &mut activity)
                .prove(&Proposition::Falsehood);
            assert_eq!(result.is_ok(), excludes);
            if let Ok(proof) = result {
                let accepted = proof_admission::accept_certificate(
                    &context,
                    &Proposition::Falsehood,
                    &assumptions,
                    &axioms,
                    &proof,
                )
                .unwrap();
                assert_eq!(accepted.assumptions.len(), 1);
                assert_eq!(accepted.assumptions[0].index, 1);
                assert_eq!(accepted.semantic_axioms.len(), 1);
                assert_eq!(accepted.semantic_axioms[0].index, 0);
            }
        }
    }
}

#[test]
fn a_later_named_result_can_bound_the_same_expression_more_tightly() {
    let mut activity = ProofSearchActivity::default();
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [dividend, divisor, first, second] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::MachineParameter { position: 1 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
        ValueOrigin::Operation(OperationId::new(2).unwrap()),
    ]
    .map(|origin| {
        let id = values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        });
        ScalarTerm::value(id, scalar_type)
    });
    let expression =
        ScalarTerm::exact_integer_remainder(integer, dividend, divisor.clone()).unwrap();
    let zero = ScalarTerm::integer(integer, integer.zero()).unwrap();
    let endpoint = IntegerValue::Unsigned(100);
    let limit = ScalarTerm::integer(integer, endpoint).unwrap();
    let assumptions = [
        Proposition::LessThan(zero, divisor),
        Proposition::LessOrEqual(second.clone(), limit.clone()),
    ];
    let axioms = [
        Proposition::Equal(first, expression.clone()),
        Proposition::Equal(second, expression.clone()),
    ];

    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let mut prover = Prover::new(&context, &assumptions, &axioms, &values, &mut activity);
    let (literal, proof) = prover.side(&expression, Direction::Upper).unwrap();
    assert_eq!(literal, endpoint);
    let goal = Proposition::LessOrEqual(expression, limit);
    proof_admission::accept_certificate(&context, &goal, &assumptions, &axioms, &proof.unwrap())
        .unwrap();
}

#[test]
fn policy_translation_maps_bounds_only_with_the_required_headroom() {
    let mut activity = ProofSearchActivity::default();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [input, result] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
    ]
    .map(|origin| {
        let id = values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        });
        ScalarTerm::value(id, scalar_type)
    });
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let add = ScalarTerm::wrapping_integer_add(integer, input.clone(), literal(2)).unwrap();
    let subtract =
        ScalarTerm::wrapping_integer_subtract(integer, input.clone(), literal(2)).unwrap();
    for (expression, guard, dependent_bound, ordinary_bound) in [
        (
            add,
            Proposition::LessOrEqual(input.clone(), literal(9)),
            Proposition::LessOrEqual(literal(2), result.clone()),
            Proposition::LessThan(result.clone(), literal(12)),
        ),
        (
            subtract,
            Proposition::LessOrEqual(literal(2), input.clone()),
            Proposition::LessOrEqual(result.clone(), literal(253)),
            Proposition::LessOrEqual(literal(0), result.clone()),
        ),
    ] {
        let axioms = [Proposition::Equal(result.clone(), expression)];
        let assumptions = [guard];
        let mut prover = Prover::new(&context, &assumptions, &axioms, &values, &mut activity);
        for goal in [&dependent_bound, &ordinary_bound] {
            let proof = prover.prove(goal).unwrap();
            proof_admission::accept_certificate(&context, goal, &assumptions, &axioms, &proof)
                .unwrap();
        }
        assert!(
            Prover::new(&context, &[], &axioms, &values, &mut activity)
                .prove(&dependent_bound)
                .is_err()
        );
    }
}

#[test]
fn wrapping_headroom_reuses_a_completed_bound_without_assuming_its_goal() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [input, result] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
    ]
    .map(|origin| {
        let id = values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        });
        ScalarTerm::value(id, scalar_type)
    });
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    let axioms = [Proposition::Equal(
        result.clone(),
        ScalarTerm::wrapping_integer_add(integer, input.clone(), literal(2)).unwrap(),
    )];
    // Searching this relation reaches the result's wrapping definition while
    // the input bound is unfinished. The later direct endpoint is already a
    // complete proof and must remain available to that headroom query.
    let assumptions = [
        Proposition::LessOrEqual(input.clone(), result.clone()),
        Proposition::LessOrEqual(input.clone(), literal(9)),
    ];
    let mut activity = ProofSearchActivity::default();
    let mut prover = Prover::new(&context, &assumptions, &axioms, &values, &mut activity);
    let ScalarTerm::Value { id, .. } = input else {
        unreachable!()
    };
    let bound = prover.bound(id, Direction::Upper).unwrap();
    assert_eq!(bound.literal, IntegerValue::Unsigned(9));
    proof_admission::accept_certificate(
        &context,
        &bound.proof.conclusion,
        &assumptions,
        &axioms,
        &bound.proof,
    )
    .unwrap();
    let goal = Proposition::LessOrEqual(literal(2), result.clone());
    let proof = prover.prove(&goal).unwrap();
    proof_admission::accept_certificate(&context, &goal, &assumptions, &axioms, &proof).unwrap();
    assert!(prover.activity.in_progress_bound_reuses > 0);

    // With no headroom premise, 254 + 2 wraps to 0. A pending attempt to
    // bound that same operand must never serve as evidence for its own guard.
    assert!(
        Prover::new(&context, &[], &axioms, &values, &mut activity)
            .prove(&goal)
            .is_err()
    );
    // A headroom proof may genuinely require both alternatives. Keep that
    // search, and refuse it when one actual alternative permits wrapping.
    for (alternative, valid) in [
        (
            Proposition::Equal(ScalarTerm::value(id, scalar_type), literal(5)),
            true,
        ),
        (
            Proposition::Equal(ScalarTerm::value(id, scalar_type), literal(254)),
            false,
        ),
    ] {
        let assumptions = [Proposition::Disjunction(vec![
            Proposition::Equal(ScalarTerm::value(id, scalar_type), literal(3)),
            alternative,
        ])];
        let proof =
            Prover::new(&context, &assumptions, &axioms, &values, &mut activity).prove(&goal);
        if valid {
            let proof = proof.unwrap();
            proof_admission::accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
                .unwrap();
        } else {
            assert!(proof.is_err(), "254 + 2 may produce zero");
        }
    }
}

#[test]
fn case_search_does_not_materialize_exhausted_or_active_cuts() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let id = values.push(ValueRow {
        scalar_type,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let input = ScalarTerm::value(id, scalar_type);
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    let cut = Proposition::Disjunction(vec![
        Proposition::LessThan(input.clone(), literal(10)),
        Proposition::LessOrEqual(literal(20), input),
    ]);
    let context = PropositionContext::from_value_types([(id, scalar_type)]).unwrap();
    let assumptions = [cut.clone()];
    let mut activity = ProofSearchActivity::default();
    let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    prover.remaining_case_branches = 2;
    let proof = prover.split(&Proposition::Truth).unwrap();
    assert_eq!(prover.remaining_case_branches, 0);
    proof_admission::accept_certificate(&context, &Proposition::Truth, &assumptions, &[], &proof)
        .unwrap();
    assert_eq!(prover.activity.disjunction_materializations, 1);
    assert_eq!(prover.activity.started, 3);

    let mut activity = ProofSearchActivity::default();
    let mut exhausted = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    exhausted.remaining_case_branches = 0;
    assert!(exhausted.split(&Proposition::Truth).is_none());
    assert_eq!(exhausted.activity.disjunction_materializations, 0);
    assert_eq!(exhausted.activity.started, 1);

    let mut activity = ProofSearchActivity::default();
    let mut active = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    active.active_cuts.push(cut);
    assert!(active.split(&Proposition::Truth).is_none());
    assert_eq!(active.activity.disjunction_materializations, 0);
    assert_eq!(active.activity.started, 1);
    let mut activity = ProofSearchActivity::default();
    let mut failed = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    let unknown = Proposition::Equal(ScalarTerm::value(id, scalar_type), literal(7));
    assert!(failed.split(&unknown).is_none());
    assert_eq!(failed.activity.disjunction_materializations, 0);
}

#[test]
fn remaining_capacity_bounds_the_exact_sum_and_its_use() {
    let mut activity = ProofSearchActivity::default();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [position, count, length, capacity, next] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::MachineParameter { position: 1 },
        ValueOrigin::MachineParameter { position: 2 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
        ValueOrigin::Operation(OperationId::new(2).unwrap()),
    ]
    .map(|origin| {
        let id = values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        });
        ScalarTerm::value(id, scalar_type)
    });
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let remaining =
        ScalarTerm::exact_integer_subtract(integer, length.clone(), position.clone()).unwrap();
    let sum = ScalarTerm::exact_integer_add(integer, position.clone(), count.clone()).unwrap();
    let sum_fits = Proposition::IntegerMathLessOrEqual(
        lift_exact_integer_order(&Proposition::LessOrEqual(sum.clone(), sum.clone()))
            .and_then(|goal| match goal {
                Proposition::IntegerMathLessOrEqual(term, _) => Some(term),
                _ => None,
            })
            .unwrap(),
        IntegerMathTerm::literal(integer.maximum_value().unwrap()),
    );
    let capacity_equation = Proposition::Equal(capacity.clone(), remaining);
    let next_equation = Proposition::Equal(next.clone(), sum);
    let assumptions = [
        Proposition::LessOrEqual(position, length.clone()),
        Proposition::LessOrEqual(count.clone(), capacity.clone()),
    ];
    let arithmetic_axioms = [capacity_equation.clone()];
    let evaluated_axioms = [capacity_equation, next_equation];
    let next_in_bounds = Proposition::LessOrEqual(next, length);
    for (goal, axioms) in [
        (&sum_fits, arithmetic_axioms.as_slice()),
        (&next_in_bounds, evaluated_axioms.as_slice()),
    ] {
        let proof = Prover::new(&context, &assumptions, axioms, &values, &mut activity)
            .prove(goal)
            .unwrap();
        proof_admission::accept_certificate(&context, goal, &assumptions, axioms, &proof).unwrap();
        let reversed = [
            assumptions[0].clone(),
            Proposition::LessOrEqual(capacity.clone(), count.clone()),
        ];
        assert!(
            Prover::new(&context, &reversed, axioms, &values, &mut activity)
                .prove(goal)
                .is_err()
        );
    }
}

#[test]
fn ieee_order_search_uses_only_connected_ordered_premises() {
    use semantic_vocabulary::{IeeeFloatComparisonKind as Kind, IeeeFloatFormat, IeeeFloatValue};
    let mut activity = ProofSearchActivity::default();
    let format = IeeeFloatFormat::Binary64;
    let scalar = ScalarType::IeeeFloat(format);
    let mut values = IdTable::default();
    let id = values.push(ValueRow {
        scalar_type: scalar,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let value = ScalarTerm::value(id, scalar);
    let literal = |bits| ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(bits));
    let relation = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    };
    let premise = relation(
        Kind::LessOrEqual,
        value.clone(),
        literal(0x3fd0_0000_0000_0000),
    );
    let goal = relation(
        Kind::LessOrEqual,
        value.clone(),
        literal(0x3fe0_0000_0000_0000),
    );
    let context = PropositionContext::from_value_types([(id, scalar)]).unwrap();
    let assumptions = [premise.clone()];
    let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
        .prove(&goal)
        .unwrap();
    proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof).unwrap();
    assert!(
        Prover::new(&context, &[], &[], &values, &mut activity)
            .prove(&goal)
            .is_err()
    );
    let complement = relation(
        Kind::NotLessThan,
        literal(0x3fd0_0000_0000_0000),
        value.clone(),
    );
    assert!(
        Prover::new(&context, &[complement], &[], &values, &mut activity)
            .prove(&goal)
            .is_err()
    );
    let nan_bound = relation(
        Kind::LessOrEqual,
        value.clone(),
        literal(0x7ff8_0000_0000_0042),
    );
    assert!(
        Prover::new(&context, &[nan_bound], &[], &values, &mut activity)
            .prove(&goal)
            .is_err()
    );
    let negative_zero = literal(0x8000_0000_0000_0000);
    let zero_bound = relation(Kind::LessOrEqual, value.clone(), literal(0));
    let zero_goal = relation(Kind::LessOrEqual, value, negative_zero);
    let assumptions = [zero_bound];
    let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
        .prove(&zero_goal)
        .unwrap();
    proof_admission::accept_certificate(&context, &zero_goal, &assumptions, &[], &proof).unwrap();
}

#[test]
fn ieee_literal_and_copy_equations_reach_the_closed_kernel_judgment() {
    use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue};
    let mut activity = ProofSearchActivity::default();
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar = ScalarType::IeeeFloat(format);
        let mut values = IdTable::default();
        let [value, copied] = [1, 2].map(|raw| {
            let id = values.push(ValueRow {
                scalar_type: scalar,
                origin: ValueOrigin::Operation(OperationId::new(raw).unwrap()),
                rigid: None,
            });
            ScalarTerm::value(id, scalar)
        });
        let [zero, quarter, half, nan] = match format {
            IeeeFloatFormat::Binary32 => {
                [0, 0x3e80_0000, 0x3f00_0000, 0x7fc0_0042].map(IeeeFloatValue::Binary32)
            }
            IeeeFloatFormat::Binary64 => [
                0,
                0x3fd0_0000_0000_0000,
                0x3fe0_0000_0000_0000,
                0x7ff8_0000_0000_0042,
            ]
            .map(IeeeFloatValue::Binary64),
        }
        .map(ScalarTerm::IeeeFloatLiteral);
        let relation = |left, right| Proposition::ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind::LessOrEqual,
            format,
            left,
            right,
        };
        let goal = Proposition::Conjunction(vec![
            relation(zero, copied.clone()),
            relation(copied.clone(), half),
        ]);
        let context = PropositionContext::from_value_types(
            values.iter().map(|(id, row)| (id, row.scalar_type)),
        )
        .unwrap();
        for (literal, expected) in [(quarter, true), (nan, false)] {
            let axioms = [
                Proposition::Equal(value.clone(), literal),
                Proposition::Equal(copied.clone(), value.clone()),
            ];
            let result = Prover::new(&context, &[], &axioms, &values, &mut activity).prove(&goal);
            assert_eq!(result.is_ok(), expected);
            if let Ok(proof) = result {
                let accepted =
                    proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof)
                        .unwrap();
                assert_eq!(accepted.semantic_axioms.len(), 2);
            }
        }
    }
}

#[test]
fn compound_exact_orders_prove_value_relations_and_reject_unsound_variants() {
    use proof_admission::MathematicalCoreDecision;
    let mut activity = ProofSearchActivity::default();

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let u64_scalar = ScalarType::Integer(u64_type);
    let mut values = IdTable::default();
    let [index, offset, length] = [0, 1, 2].map(|position| {
        let id = values.push(ValueRow {
            scalar_type: u64_scalar,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        });
        ScalarTerm::value(id, u64_scalar)
    });
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let lit = |n| ScalarTerm::integer(u64_type, IntegerValue::Unsigned(n)).unwrap();
    let add = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_add(u64_type, a.clone(), b.clone()).unwrap()
    };
    let sub = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_subtract(u64_type, a.clone(), b.clone()).unwrap()
    };

    let index_below_length = Proposition::LessThan(index.clone(), length.clone());
    let offset_below_length = Proposition::LessThan(offset.clone(), length.clone());
    let one = lit(1);
    for (assumption, goal) in [
        (
            Proposition::LessThan(add(&index, &one), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessThan(add(&one, &index), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessOrEqual(add(&add(&index, &one), &one), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessThan(add(&index, &offset), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessThan(add(&index, &offset), length.clone()),
            &offset_below_length,
        ),
        (
            Proposition::Equal(add(&index, &one), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessOrEqual(index.clone(), sub(&length, &one)),
            &index_below_length,
        ),
        (
            Proposition::LessThan(index.clone(), sub(&length, &offset)),
            &index_below_length,
        ),
        (
            Proposition::LessOrEqual(add(&index, &one), sub(&length, &one)),
            &index_below_length,
        ),
    ] {
        let assumptions = [assumption];
        let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
            .prove(goal)
            .unwrap_or_else(|err| panic!("failed on {assumptions:?} |- {goal:?}: {err}"));
        let accepted =
            proof_admission::accept_certificate(&context, goal, &assumptions, &[], &proof).unwrap();
        assert!(
            matches!(
                accepted.mathematical_core,
                MathematicalCoreDecision::Judged(_)
            ),
            "{:?}",
            accepted.mathematical_core
        );
    }

    // Insufficient headroom, wrapping arithmetic, and saturating arithmetic must fail closed.
    let wrapping_plus_one =
        ScalarTerm::wrapping_integer_add(u64_type, index.clone(), one.clone()).unwrap();
    let saturating_plus_one =
        ScalarTerm::saturating_integer_add(u64_type, index.clone(), one.clone()).unwrap();
    for (assumption, invalid_goal) in [
        (
            Proposition::LessThan(index.clone(), length.clone()),
            Proposition::LessThan(add(&index, &one), length.clone()),
        ),
        (
            Proposition::LessOrEqual(add(&index, &one), length.clone()),
            Proposition::LessThan(add(&index, &one), length.clone()),
        ),
        (
            Proposition::LessThan(wrapping_plus_one, length.clone()),
            index_below_length.clone(),
        ),
        (
            Proposition::LessThan(saturating_plus_one, length.clone()),
            index_below_length,
        ),
    ] {
        let assumptions = [assumption];
        assert!(
            Prover::new(&context, &assumptions, &[], &values, &mut activity)
                .prove(&invalid_goal)
                .is_err(),
            "unexpectedly proved {assumptions:?} |- {invalid_goal:?}"
        );
    }

    // Signed offsets may be negative unless proved `>= 0` or `> 0`.
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let i64_scalar = ScalarType::Integer(i64_type);
    let mut signed_values = IdTable::default();
    let [a, d, b] = [0, 1, 2].map(|position| {
        let id = signed_values.push(ValueRow {
            scalar_type: i64_scalar,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        });
        ScalarTerm::value(id, i64_scalar)
    });
    let signed_context = PropositionContext::from_value_types(
        signed_values.iter().map(|(id, row)| (id, row.scalar_type)),
    )
    .unwrap();
    let signed_zero = ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).unwrap();
    let signed_sum = ScalarTerm::exact_integer_add(i64_type, a.clone(), d.clone()).unwrap();
    let signed_diff = ScalarTerm::exact_integer_subtract(i64_type, b.clone(), d.clone()).unwrap();
    let signed_goal = Proposition::LessOrEqual(a.clone(), b.clone());
    let signed_strict_goal = Proposition::LessThan(a.clone(), b.clone());
    for unguarded in [
        Proposition::LessThan(signed_sum.clone(), b.clone()),
        Proposition::LessOrEqual(a, signed_diff),
    ] {
        assert!(
            Prover::new(
                &signed_context,
                &[unguarded],
                &[],
                &signed_values,
                &mut activity
            )
            .prove(&signed_goal)
            .is_err()
        );
    }
    let guarded = [
        Proposition::LessThan(signed_sum, b),
        Proposition::LessOrEqual(signed_zero, d),
    ];
    let proof = Prover::new(
        &signed_context,
        &guarded,
        &[],
        &signed_values,
        &mut activity,
    )
    .prove(&signed_strict_goal)
    .unwrap();
    let accepted = proof_admission::accept_certificate(
        &signed_context,
        &signed_strict_goal,
        &guarded,
        &[],
        &proof,
    )
    .unwrap();
    assert!(matches!(
        accepted.mathematical_core,
        MathematicalCoreDecision::Judged(_)
    ));
}

#[test]
fn compound_constant_offsets_tighten_literal_and_chained_bounds_without_poisoning_ssa_results() {
    let mut activity = ProofSearchActivity::default();
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u8_scalar = ScalarType::Integer(u8_type);
    let mut values = IdTable::default();
    let [index_id, limit_id, count_id, next_id] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::MachineParameter { position: 1 },
        ValueOrigin::MachineParameter { position: 2 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
    ]
    .map(|origin| {
        values.push(ValueRow {
            scalar_type: u8_scalar,
            origin,
            rigid: None,
        })
    });
    let [index, limit, count, next] =
        [index_id, limit_id, count_id, next_id].map(|id| ScalarTerm::value(id, u8_scalar));
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let lit = |n| ScalarTerm::integer(u8_type, IntegerValue::Unsigned(n)).unwrap();
    let add = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_add(u8_type, a.clone(), b.clone()).unwrap()
    };
    let sub = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_subtract(u8_type, a.clone(), b.clone()).unwrap()
    };

    // `index + 1 < 4` tightens `index` all the way to `<= 2`, not just `<= 3`.
    let assumptions = [Proposition::LessThan(add(&index, &lit(1)), lit(4))];
    let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    assert_eq!(
        prover.literal_bounds(index_id),
        (None, Some(IntegerValue::Unsigned(2)))
    );
    let before_copies = prover.activity.bound_certificate_copies;
    assert_eq!(
        prover.literal_bounds(index_id),
        (None, Some(IntegerValue::Unsigned(2)))
    );
    assert_eq!(prover.activity.bound_certificate_copies, before_copies);
    let exact_goal = Proposition::LessOrEqual(index.clone(), lit(2));
    {
        let Slot::Done(Some(bound)) =
            &prover.bounds.get(index_id).unwrap()[Direction::Upper.index()]
        else {
            panic!("the offset bound must retain its completed certificate");
        };
        let mut translated = &bound.proof;
        for expected_endpoint in [1, 0] {
            let ProofRule::IntegerOrderSubstitution {
                relation, endpoint, ..
            } = &translated.rule
            else {
                panic!(
                    "translation must restore both exact endpoints: {:?}",
                    translated.rule
                );
            };
            assert_eq!(*endpoint, expected_endpoint);
            translated = relation.as_ref();
        }
        let ProofRule::IntegerMathOrderTranslation { relation, addend } = &translated.rule else {
            panic!(
                "the exact offset must use mathematical order translation: {:?}",
                translated.rule
            );
        };
        assert_eq!(addend, &IntegerMathTerm::literal(IntegerValue::Signed(-1)));
        let ProofRule::IntegerOrderSubstitution {
            relation: original,
            endpoint: 1,
            ..
        } = &relation.rule
        else {
            panic!(
                "the original upper order must retain its literal endpoint: {:?}",
                relation.rule
            );
        };
        assert_eq!(
            original.as_ref(),
            &node(
                Proposition::LessOrEqual(add(&index, &lit(1)), lit(3)),
                ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(node(
                        assumptions[0].clone(),
                        ProofRule::Assumption { index: 0 }
                    )),
                },
            )
        );
        proof_admission::accept_certificate(&context, &exact_goal, &assumptions, &[], &bound.proof)
            .unwrap();
    }
    let proof = prover.prove(&exact_goal).unwrap();
    proof_admission::accept_certificate(&context, &exact_goal, &assumptions, &[], &proof).unwrap();
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(index.clone(), lit(1)))
            .is_err()
    );

    // `index + 2 <= limit` with `limit <= 10` yields `index <= 8`, and
    // `0 < count - 1` yields `2 <= count`.
    let chained = [
        Proposition::LessOrEqual(add(&index, &lit(2)), limit.clone()),
        Proposition::LessOrEqual(limit, lit(10)),
        Proposition::LessThan(lit(0), sub(&count, &lit(1))),
    ];
    let mut prover = Prover::new(&context, &chained, &[], &values, &mut activity);
    for goal in [
        Proposition::LessOrEqual(index.clone(), lit(8)),
        Proposition::LessOrEqual(lit(2), count.clone()),
    ] {
        let proof = prover.prove(&goal).unwrap();
        proof_admission::accept_certificate(&context, &goal, &chained, &[], &proof).unwrap();
    }
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(index.clone(), lit(7)))
            .is_err()
    );
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(lit(3), count))
            .is_err()
    );

    // Querying `index`'s upper bound first must not poison an SSA result `next = index + 1`.
    let ssa_assumptions = [Proposition::LessOrEqual(index.clone(), lit(5))];
    let ssa_axioms = [Proposition::Equal(next.clone(), add(&index, &lit(1)))];
    let mut prover = Prover::new(
        &context,
        &ssa_assumptions,
        &ssa_axioms,
        &values,
        &mut activity,
    );
    assert_eq!(
        prover.literal_bounds(index_id),
        (None, Some(IntegerValue::Unsigned(5)))
    );
    let next_goal = Proposition::LessOrEqual(next, lit(6));
    let proof = prover.prove(&next_goal).unwrap();
    proof_admission::accept_certificate(
        &context,
        &next_goal,
        &ssa_assumptions,
        &ssa_axioms,
        &proof,
    )
    .unwrap();
}

#[test]
fn numeric_conclusions_use_checked_implications_and_keep_their_premises() {
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let mut values = IdTable::default();
    let condition = values.push(ValueRow {
        scalar_type: ScalarType::Boolean,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let read = values.push(ValueRow {
        scalar_type: ScalarType::Integer(byte),
        origin: ValueOrigin::MachineParameter { position: 1 },
        rigid: None,
    });
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let premise = Proposition::Equal(
        ScalarTerm::value(condition, ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    );
    let goal = Proposition::LessOrEqual(
        ScalarTerm::value(read, ScalarType::Integer(byte)),
        ScalarTerm::integer(byte, IntegerValue::Unsigned(127)).unwrap(),
    );
    let implication = Proposition::Implication {
        premise: Box::new(premise.clone()),
        conclusion: Box::new(goal.clone()),
    };
    let assumptions = [premise.clone(), implication.clone()];
    let mut activity = ProofSearchActivity::default();
    let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
        .prove(&goal)
        .unwrap();
    let accepted =
        proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof).unwrap();
    assert_eq!(accepted.assumptions.len(), 2);
    let observed = ScalarTerm::value(read, ScalarType::Integer(byte));
    let literal = ScalarTerm::integer(byte, IntegerValue::Unsigned(7)).unwrap();
    let equality = Proposition::Equal(observed.clone(), literal.clone());
    let reversed = [
        premise.clone(),
        Proposition::Implication {
            premise: Box::new(premise.clone()),
            conclusion: Box::new(Proposition::Equal(literal, observed)),
        },
    ];
    let proof = Prover::new(&context, &reversed, &[], &values, &mut activity)
        .prove(&equality)
        .unwrap();
    let accepted =
        proof_admission::accept_certificate(&context, &equality, &reversed, &[], &proof).unwrap();
    assert_eq!(accepted.assumptions.len(), 2);

    for missing in [
        vec![premise],
        vec![implication],
        vec![Proposition::Implication {
            premise: Box::new(goal.clone()),
            conclusion: Box::new(goal.clone()),
        }],
    ] {
        assert!(
            Prover::new(&context, &missing, &[], &values, &mut activity)
                .prove(&goal)
                .is_err()
        );
    }
}

#[test]
fn irrelevant_subtraction_endpoints_do_not_start_sign_searches() {
    let integer = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [subject, other] = [0, 1].map(|position| {
        let id = values.push(ValueRow {
            scalar_type: scalar,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        });
        ScalarTerm::value(id, scalar)
    });
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let ScalarTerm::Value { id, .. } = subject else {
        unreachable!();
    };
    let three = ScalarTerm::integer(integer, IntegerValue::Signed(3)).unwrap();
    let difference =
        ScalarTerm::exact_integer_subtract(integer, other.clone(), subject.clone()).unwrap();
    let assumptions = [Proposition::LessOrEqual(three, difference)];
    let mut activity = ProofSearchActivity::default();
    let bounds =
        Prover::new(&context, &assumptions, &[], &values, &mut activity).literal_bounds(id);
    assert_eq!(bounds, (None, None));
    assert_eq!(activity.sign_queries, 0, "the offset is not the minuend");

    let sum = ScalarTerm::exact_integer_add(integer, subject.clone(), other.clone()).unwrap();
    let difference = ScalarTerm::exact_integer_subtract(integer, subject.clone(), other).unwrap();
    let assumptions = [Proposition::LessOrEqual(sum, difference)];
    let mut activity = ProofSearchActivity::default();
    let steps =
        Prover::new(&context, &assumptions, &[], &values, &mut activity).steps(&subject, &subject);
    assert!(
        steps.is_empty(),
        "an order step cannot return its starting value"
    );
    assert_eq!(
        activity.sign_queries, 0,
        "a self-step needs no offset proof"
    );
}

#[test]
fn unsigned_carrier_endpoint_uses_the_existing_checked_primitive() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let id = values.push(ValueRow {
        scalar_type: scalar,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let context = PropositionContext::from_value_types([(id, scalar)]).unwrap();
    let goal = Proposition::LessOrEqual(
        ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap(),
        ScalarTerm::value(id, scalar),
    );
    let mut activity = ProofSearchActivity::default();
    let proof = Prover::new(&context, &[], &[], &values, &mut activity)
        .prove(&goal)
        .unwrap();
    assert!(matches!(
        proof.rule,
        ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound)
    ));
    let accepted = proof_admission::accept_certificate(&context, &goal, &[], &[], &proof).unwrap();
    assert!(accepted.assumptions.is_empty());
    assert!(accepted.semantic_axioms.is_empty());

    let signed = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let scalar = ScalarType::Integer(signed);
    let mut values = IdTable::default();
    let id = values.push(ValueRow {
        scalar_type: scalar,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let context = PropositionContext::from_value_types([(id, scalar)]).unwrap();
    let goal = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).unwrap(),
        ScalarTerm::value(id, scalar),
    );
    assert!(
        Prover::new(&context, &[], &[], &values, &mut activity)
            .prove(&goal)
            .is_err()
    );
}

#[test]
fn falsehood_implications_do_not_restart_the_same_contradiction_search() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let id = values.push(ValueRow {
        scalar_type: scalar,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let context = PropositionContext::from_value_types([(id, scalar)]).unwrap();
    let value = ScalarTerm::value(id, scalar);
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    let premise = Proposition::LessOrEqual(value.clone(), literal(3));
    let excludes = Proposition::Implication {
        premise: Box::new(premise.clone()),
        conclusion: Box::new(Proposition::Falsehood),
    };
    let mut activity = ProofSearchActivity::default();
    assert!(
        Prover::new(
            &context,
            std::slice::from_ref(&excludes),
            &[],
            &values,
            &mut activity,
        )
        .prove(&Proposition::Falsehood)
        .is_err()
    );
    assert_eq!(activity.contradiction_searches, 1);

    // A proved antecedent still eliminates its implication, with both actual
    // assumptions cited in the certificate.
    let assumptions = [excludes.clone(), premise];
    let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
        .prove(&Proposition::Falsehood)
        .unwrap();
    let accepted = proof_admission::accept_certificate(
        &context,
        &Proposition::Falsehood,
        &assumptions,
        &[],
        &proof,
    )
    .unwrap();
    assert_eq!(accepted.assumptions.len(), 2);

    // The outer contradiction pass remains available when a different pair
    // of arithmetic assumptions contradicts. No antecedent proves itself.
    let assumptions = [
        excludes,
        Proposition::LessOrEqual(value.clone(), literal(5)),
        Proposition::LessOrEqual(literal(8), value),
    ];
    let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
        .prove(&Proposition::Falsehood)
        .unwrap();
    proof_admission::accept_certificate(
        &context,
        &Proposition::Falsehood,
        &assumptions,
        &[],
        &proof,
    )
    .unwrap();
}

#[test]
fn contradictory_strict_orders_discharge_an_unrelated_goal_with_an_admitted_certificate() {
    use semantic_vocabulary::{IntegerType, IntegerValue};
    let mut activity = ProofSearchActivity::default();
    let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar = ScalarType::Integer(signed);
    let mut values = IdTable::default();
    let [x, y, z] = [1, 2, 3].map(|raw| {
        let id = values.push(ValueRow {
            scalar_type: scalar,
            origin: ValueOrigin::Operation(OperationId::new(raw).unwrap()),
            rigid: None,
        });
        ScalarTerm::value(id, scalar)
    });
    let literal = |n: i128| {
        ScalarTerm::integer(
            signed,
            IntegerValue::for_sign(IntegerSign::Signed, n).unwrap(),
        )
        .unwrap()
    };
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let goal = Proposition::LessOrEqual(z, literal(0));
    for (y_value, inconsistent) in [(-100, true), (10, false)] {
        let assumptions = [
            Proposition::LessThan(literal(0), x.clone()),
            Proposition::LessThan(x.clone(), y.clone()),
            Proposition::Equal(y.clone(), literal(y_value)),
        ];
        let result = Prover::new(&context, &assumptions, &[], &values, &mut activity).prove(&goal);
        assert_eq!(result.is_ok(), inconsistent);
        if let Ok(proof) = result {
            proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof)
                .unwrap();
        }
    }
}

#[test]
fn dense_order_arrivals_copy_only_the_bounds_the_certificate_uses() {
    // The relations have quadratic size, but they all carry the same endpoint.
    // Replaying equal endpoints must not copy a growing certificate per edge.
    for count in [4, 8, 16] {
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let mut values = IdTable::default();
        let inputs: Vec<_> = (0..count)
            .map(|position| {
                let id = values.push(ValueRow {
                    scalar_type,
                    origin: ValueOrigin::MachineParameter {
                        position: position as u32,
                    },
                    rigid: None,
                });
                ScalarTerm::value(id, scalar_type)
            })
            .collect();
        let context = PropositionContext::from_value_types(
            values.iter().map(|(id, row)| (id, row.scalar_type)),
        )
        .unwrap();
        let limit = ScalarTerm::integer(integer, IntegerValue::Unsigned(9)).unwrap();
        let goal = Proposition::LessOrEqual(inputs[count - 1].clone(), limit.clone());
        let mut assumptions = vec![Proposition::LessOrEqual(inputs[0].clone(), limit)];
        for (left, input) in inputs.iter().enumerate() {
            for (right, other) in inputs.iter().enumerate() {
                if left != right {
                    assumptions.push(Proposition::LessOrEqual(input.clone(), other.clone()));
                }
            }
        }
        let mut activity = ProofSearchActivity::default();
        let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
        let proof = prover.prove(&goal).unwrap();
        let accepted =
            proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof)
                .unwrap();
        assert!(accepted.assumptions.iter().any(|used| used.index == 0));
        assert!(accepted.assumptions.iter().any(|used| used.index > 0));
        assert!(
            prover.activity.bound_certificate_copies <= count,
            "{} copies for {count} subjects",
            prover.activity.bound_certificate_copies,
        );

        // Without the literal arrival, all subjects can be 10. Keep this
        // semantic falsifier small; the growing cases above measure work.
        if count == 4 {
            assert!(
                Prover::new(
                    &context,
                    &assumptions[1..],
                    &[],
                    &values,
                    &mut ProofSearchActivity::default(),
                )
                .prove(&goal)
                .is_err()
            );
        }
    }
}

#[test]
fn discarded_compound_endpoints_keep_queries_without_copying_their_bound() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [subject_id, limit_id, offset_id] = [0, 1, 2].map(|position| {
        values.push(ValueRow {
            scalar_type,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        })
    });
    let [subject, limit, offset] =
        [subject_id, limit_id, offset_id].map(|id| ScalarTerm::value(id, scalar_type));
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let difference =
        ScalarTerm::exact_integer_subtract(integer, limit.clone(), offset.clone()).unwrap();
    let assumptions = [
        Proposition::LessOrEqual(subject.clone(), literal(5)),
        Proposition::LessOrEqual(limit.clone(), literal(9)),
        Proposition::LessOrEqual(literal(1), offset.clone()),
        Proposition::LessOrEqual(subject.clone(), difference),
    ];
    let mut activity = ProofSearchActivity::default();
    let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    // Hold the offset's completed proof fixed. The measured query must still
    // prove its sign and settle the limit before discarding the weaker result.
    assert_eq!(
        prover.bound_literal(offset_id, Direction::Lower),
        Some(IntegerValue::Unsigned(1))
    );
    assert!(matches!(
        prover.bounds.get(limit_id).unwrap()[Direction::Upper.index()],
        Slot::Unknown
    ));
    let before_copies = prover.activity.bound_certificate_copies;
    let before_signs = prover.activity.sign_queries;
    assert_eq!(
        prover.bound_literal(subject_id, Direction::Upper),
        Some(IntegerValue::Unsigned(5))
    );
    assert_eq!(prover.activity.sign_queries - before_signs, 1);
    assert!(matches!(
        &prover.bounds.get(limit_id).unwrap()[Direction::Upper.index()],
        Slot::Done(Some(bound)) if bound.literal == IntegerValue::Unsigned(9)
    ));
    // The sign query consumes the offset's proof. The unused limit proof is
    // not copied; its completed endpoint remains available to later queries.
    assert_eq!(prover.activity.bound_certificate_copies - before_copies, 1);
    let goal = Proposition::LessOrEqual(subject.clone(), literal(5));
    let proof = prover.prove(&goal).unwrap();
    let accepted =
        proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof).unwrap();
    assert_eq!(
        accepted
            .assumptions
            .iter()
            .map(|used| used.index)
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(subject, literal(4)))
            .is_err()
    );
}

#[test]
fn selected_relation_bounds_keep_strict_steps_and_equality_direction() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let inputs: Vec<_> = (0..2)
        .map(|position| {
            let id = values.push(ValueRow {
                scalar_type,
                origin: ValueOrigin::MachineParameter { position },
                rigid: None,
            });
            ScalarTerm::value(id, scalar_type)
        })
        .collect();
    let (left, right) = (&inputs[0], &inputs[1]);
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    for direction in [Direction::Upper, Direction::Lower] {
        for relation in [
            Proposition::LessThan(left.clone(), right.clone()),
            Proposition::Equal(left.clone(), right.clone()),
            Proposition::Equal(right.clone(), left.clone()),
        ] {
            let strict = matches!(relation, Proposition::LessThan(..));
            let (subject, bounded) = match direction {
                Direction::Upper => (left, right),
                Direction::Lower => (right, left),
            };
            let endpoint = if strict {
                match direction {
                    Direction::Upper => 8,
                    Direction::Lower => 10,
                }
            } else {
                9
            };
            let assumptions = [relation, Prover::oriented(bounded, literal(9), direction)];
            let goal = Prover::oriented(subject, literal(endpoint), direction);
            let mut activity = ProofSearchActivity::default();
            let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
                .prove(&goal)
                .unwrap();
            let accepted =
                proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof)
                    .unwrap();
            assert!(accepted.assumptions.iter().any(|used| used.index == 0));
            assert!(accepted.assumptions.iter().any(|used| used.index == 1));
            let beyond = match direction {
                Direction::Upper => endpoint - 1,
                Direction::Lower => endpoint + 1,
            };
            let unproved = Prover::oriented(subject, literal(beyond), direction);
            assert!(
                Prover::new(&context, &assumptions, &[], &values, &mut activity)
                    .prove(&unproved)
                    .is_err()
            );
        }
    }
}

/// A value-to-value order cannot finish `refute`'s existing closed-endpoint
/// chain. Refusing before the opposite search must copy no bound evidence.
#[test]
fn refutation_without_a_closed_endpoint_skips_bound_work() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let inputs: Vec<_> = (0..2)
        .map(|position| {
            let id = values.push(ValueRow {
                scalar_type,
                origin: ValueOrigin::MachineParameter { position },
                rigid: None,
            });
            ScalarTerm::value(id, scalar_type)
        })
        .collect();
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let assumptions = [Proposition::LessOrEqual(
        inputs[0].clone(),
        inputs[1].clone(),
    )];
    let local = node(assumptions[0].clone(), ProofRule::Assumption { index: 0 });
    let mut activity = ProofSearchActivity::default();
    let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    assert!(
        prover
            .refute(&assumptions[0], |_, _| Some(local), &Proposition::Falsehood)
            .is_none()
    );
    let copied_for_refutation = prover.activity.bound_certificate_copies;
    assert_eq!(prover.remaining_case_branches, CASE_BRANCH_BUDGET);
    assert!(prover.prove(&Proposition::Falsehood).is_err());
    assert_eq!(copied_for_refutation, 0);
}

/// Skipping an unusable first order must retain the later literal-endpoint
/// contradiction and its real premises, in either orientation. Removing
/// either conflicting premise leaves a consistent context and must refuse.
#[test]
fn literal_endpoint_refutation_keeps_the_actual_conflicting_premises() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let inputs: Vec<_> = (0..2)
        .map(|position| {
            let id = values.push(ValueRow {
                scalar_type,
                origin: ValueOrigin::MachineParameter { position },
                rigid: None,
            });
            ScalarTerm::value(id, scalar_type)
        })
        .collect();
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    let upper = Proposition::LessOrEqual(inputs[0].clone(), literal(3));
    let lower = Proposition::LessOrEqual(literal(4), inputs[0].clone());
    for reversed in [false, true] {
        let mut assumptions = vec![Proposition::LessOrEqual(
            inputs[0].clone(),
            inputs[1].clone(),
        )];
        if reversed {
            assumptions.extend([lower.clone(), upper.clone()]);
        } else {
            assumptions.extend([upper.clone(), lower.clone()]);
        }
        let mut activity = ProofSearchActivity::default();
        let proof = Prover::new(&context, &assumptions, &[], &values, &mut activity)
            .prove(&Proposition::Falsehood)
            .unwrap();
        let accepted = proof_admission::accept_certificate(
            &context,
            &Proposition::Falsehood,
            &assumptions,
            &[],
            &proof,
        )
        .unwrap();
        let mut cited: Vec<_> = accepted.assumptions.iter().map(|used| used.index).collect();
        cited.sort_unstable();
        assert_eq!(cited, [1, 2]);
        assert!(accepted.semantic_axioms.is_empty());
        for missing in [1, 2] {
            let mut remaining = assumptions.clone();
            remaining.remove(missing);
            assert!(
                Prover::new(
                    &context,
                    &remaining,
                    &[],
                    &values,
                    &mut ProofSearchActivity::default(),
                )
                .prove(&Proposition::Falsehood)
                .is_err()
            );
        }
    }
}

#[test]
fn a_compound_direct_endpoint_tightens_a_raw_tie_by_its_strict_step() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [subject_id, limit_id, offset_id] = [0, 1, 2].map(|position| {
        values.push(ValueRow {
            scalar_type,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        })
    });
    let [subject, limit, offset] =
        [subject_id, limit_id, offset_id].map(|id| ScalarTerm::value(id, scalar_type));
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let difference =
        ScalarTerm::exact_integer_subtract(integer, limit.clone(), offset.clone()).unwrap();
    let assumptions = [
        Proposition::LessOrEqual(subject.clone(), literal(5)),
        Proposition::LessOrEqual(limit, literal(5)),
        Proposition::LessOrEqual(literal(1), offset),
        Proposition::LessOrEqual(subject.clone(), difference),
    ];
    let mut activity = ProofSearchActivity::default();
    let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    // The raw limit ties the retained five. The positive offset makes the
    // final endpoint four, so filtering before that strict step loses proof.
    assert_eq!(
        prover.bound_literal(subject_id, Direction::Upper),
        Some(IntegerValue::Unsigned(4)),
    );
    let goal = Proposition::LessOrEqual(subject.clone(), literal(4));
    let proof = prover.prove(&goal).unwrap();
    let accepted =
        proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof).unwrap();
    for required in [1, 2, 3] {
        assert!(
            accepted
                .assumptions
                .iter()
                .any(|used| used.index == required)
        );
    }
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(subject, literal(3)))
            .is_err()
    );
}

#[test]
fn discarded_compound_orders_preserve_queries_without_materializing_facts() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [subject_id, limit_id, offset_id] = [0, 1, 2].map(|position| {
        values.push(ValueRow {
            scalar_type,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        })
    });
    let [subject, limit, offset] =
        [subject_id, limit_id, offset_id].map(|id| ScalarTerm::value(id, scalar_type));
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    let difference =
        ScalarTerm::exact_integer_subtract(integer, limit.clone(), offset.clone()).unwrap();
    let assumptions = [
        Proposition::LessOrEqual(subject.clone(), literal(5)),
        Proposition::LessOrEqual(limit.clone(), literal(9)),
        Proposition::LessOrEqual(literal(1), offset),
        Proposition::LessOrEqual(subject.clone(), difference),
    ];
    let mut activity = ProofSearchActivity::default();
    let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
    assert_eq!(
        prover.bound_literal(offset_id, Direction::Lower),
        Some(IntegerValue::Unsigned(1))
    );
    let before_nodes = prover.activity.compound_order_fact_nodes;
    let before_signs = prover.activity.sign_queries;
    let before_copies = prover.activity.bound_certificate_copies;
    assert_eq!(
        prover.bound_literal(subject_id, Direction::Upper),
        Some(IntegerValue::Unsigned(5))
    );
    assert_eq!(prover.activity.sign_queries - before_signs, 1);
    assert_eq!(prover.activity.bound_certificate_copies - before_copies, 1);
    assert!(
        matches!(&prover.bounds.get(limit_id).unwrap()[Direction::Upper.index()],
        Slot::Done(Some(bound)) if bound.literal == IntegerValue::Unsigned(9))
    );
    assert_eq!(prover.activity.compound_order_fact_nodes - before_nodes, 0);
    let goal = Proposition::LessOrEqual(subject.clone(), literal(5));
    let proof = prover.prove(&goal).unwrap();
    let accepted =
        proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof).unwrap();
    assert_eq!(
        accepted
            .assumptions
            .iter()
            .map(|used| used.index)
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(subject.clone(), literal(4)))
            .is_err()
    );

    // The same order owner must retain forward and reversed equality proofs,
    // and keep the exact offset rather than weakening it to a monotone bound.
    let sum = ScalarTerm::exact_integer_add(integer, subject.clone(), literal(1)).unwrap();
    for equality in [
        Proposition::Equal(sum.clone(), limit.clone()),
        Proposition::Equal(limit.clone(), sum),
    ] {
        for direction in [Direction::Upper, Direction::Lower] {
            let assumptions = [
                equality.clone(),
                Prover::oriented(&limit, literal(9), direction),
            ];
            let mut activity = ProofSearchActivity::default();
            let mut prover = Prover::new(&context, &assumptions, &[], &values, &mut activity);
            let goal = Prover::oriented(&subject, literal(8), direction);
            let proof = prover.prove(&goal).unwrap();
            let accepted =
                proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof)
                    .unwrap();
            for index in [0, 1] {
                assert!(accepted.assumptions.iter().any(|used| used.index == index));
            }
            assert!(prover.activity.compound_order_fact_nodes > 0);
            let beyond = match direction {
                Direction::Upper => 7,
                Direction::Lower => 9,
            };
            assert!(
                prover
                    .prove(&Prover::oriented(&subject, literal(beyond), direction))
                    .is_err()
            );
        }
    }
}
