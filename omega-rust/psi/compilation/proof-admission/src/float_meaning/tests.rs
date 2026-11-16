use super::*;
use crate::{
    AcceptedProofRule, MathematicalCoreDecision, ProofNode, ProofRule, accept_certificate,
};
use semantic_vocabulary::{
    ContentPlaceVersion, PlaceId, PropositionContext, ScalarType, StructuralPlaceKind, ValueId,
};

#[path = "model.rs"]
mod model;

fn value(raw: u64, format: IeeeFloatFormat) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::IeeeFloat(format))
}

fn widening(source: ScalarTerm, wide: ScalarTerm) -> Proposition {
    let convert = Normalization::new().unwrap().convert;
    Proposition::FloatMeaningEqual(
        FloatMeaningClause::result_equation(wide, convert, IeeeFloatFormat::Binary64, [source])
            .unwrap(),
    )
}

fn goal(source: ScalarTerm, through: IeeeFloatFormat) -> Proposition {
    Proposition::FloatMeaningEqual(
        FloatMeaningClause::conversion_roundtrip(source, through).unwrap(),
    )
}

fn certificate(goal: &Proposition, equations: &[Proposition]) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::FloatMeaningNormalization(
            equations
                .iter()
                .enumerate()
                .map(|(index, equation)| ProofNode {
                    conclusion: equation.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                })
                .collect(),
        ),
    }
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types([
        (
            ValueId::new(1).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ),
        (
            ValueId::new(2).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ),
        (
            ValueId::new(3).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ),
    ])
    .unwrap()
}

fn literal_equality(left: IeeeFloatValue, right: IeeeFloatValue) -> Proposition {
    let projection = |value: IeeeFloatValue| {
        let operation = match value.format() {
            IeeeFloatFormat::Binary32 => {
                numerics::float_projection::FloatProjectionOperation::Meaning32
            }
            IeeeFloatFormat::Binary64 => {
                numerics::float_projection::FloatProjectionOperation::Meaning64
            }
        };
        FloatMeaningClauseValue::Projection {
            source: ScalarTerm::IeeeFloatLiteral(value),
            contract: operation.contract_identity(),
        }
    };
    Proposition::FloatMeaningEqual(FloatMeaningClause {
        values: vec![projection(left), projection(right)],
        left: semantic_vocabulary::FloatMeaningSlot(0),
        right: semantic_vocabulary::FloatMeaningSlot(1),
    })
}

#[test]
fn roundtrip_certificate_cites_only_the_live_equation_and_refuses_forged_children() {
    let narrow = value(1, IeeeFloatFormat::Binary32);
    let wide = value(2, IeeeFloatFormat::Binary64);
    let equation = widening(narrow, wide.clone());
    let desired = goal(wide.clone(), IeeeFloatFormat::Binary32);
    let proof = certificate(&desired, std::slice::from_ref(&equation));
    let axioms = [equation.clone(), Proposition::Truth];
    let accepted = accept_certificate(&context(), &desired, &[], &axioms, &proof).unwrap();
    assert_eq!(accepted.semantic_axioms.len(), 1);
    assert_eq!(accepted.semantic_axioms[0].index, 0);
    assert!(accepted.assumptions.is_empty());
    assert!(
        accepted
            .rules
            .contains(&AcceptedProofRule::FloatMeaningNormalization)
    );
    assert!(AcceptedProofRule::FloatMeaningNormalization.is_provisional());
    assert!(matches!(
        accepted.mathematical_core,
        MathematicalCoreDecision::Refused(_)
    ));
    let missing = certificate(&desired, &[]);
    assert!(accept_certificate(&context(), &desired, &[], &axioms, &missing).is_err());
    let mut forged = proof.clone();
    let ProofRule::FloatMeaningNormalization(children) = &mut forged.rule else {
        unreachable!()
    };
    children[0].rule = ProofRule::SemanticAxiom { index: 1 };
    assert!(accept_certificate(&context(), &desired, &[], &axioms, &forged).is_err());
    let mut unrelated = proof.clone();
    unrelated.conclusion = goal(
        value(3, IeeeFloatFormat::Binary64),
        IeeeFloatFormat::Binary32,
    );
    assert!(
        accept_certificate(&context(), &unrelated.conclusion, &[], &axioms, &unrelated).is_err()
    );
    let arbitrary_wide = widening(value(3, IeeeFloatFormat::Binary64), wide);
    assert!(
        accept_certificate(
            &context(),
            &desired,
            &[],
            std::slice::from_ref(&arbitrary_wide),
            &certificate(&desired, std::slice::from_ref(&arbitrary_wide))
        )
        .is_err()
    );
    let wrong_type = goal(
        value(2, IeeeFloatFormat::Binary32),
        IeeeFloatFormat::Binary64,
    );
    assert!(
        accept_certificate(
            &context(),
            &wrong_type,
            &[],
            &[],
            &certificate(&wrong_type, &[])
        )
        .is_err()
    );
    let mut wrong_catalog = equation;
    let Proposition::FloatMeaningEqual(clause) = &mut wrong_catalog else {
        unreachable!()
    };
    for row in &mut clause.values {
        if let FloatMeaningClauseValue::Application { contract, .. } = row {
            contract.commitment[0] ^= 1;
        }
    }
    assert!(check_float_meaning_normalization(&desired, [&wrong_catalog]).is_err());
}

#[test]
fn meaning_normalization_keeps_storage_versions_and_ieee_relations_separate() {
    let root = PlaceId::new(1).unwrap();
    let field = |version| ScalarTerm::IeeeFloatField {
        version,
        root,
        path: Vec::new(),
        format: IeeeFloatFormat::Binary64,
    };
    let current = field(ContentPlaceVersion::Current);
    let entry = field(ContentPlaceVersion::Entry);
    let equation = widening(value(1, IeeeFloatFormat::Binary32), current.clone());
    let context = PropositionContext::from_value_types_and_places(
        [(
            ValueId::new(1).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        )],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .unwrap();
    let current_goal = goal(current, IeeeFloatFormat::Binary32);
    assert!(
        accept_certificate(
            &context,
            &current_goal,
            &[],
            std::slice::from_ref(&equation),
            &certificate(&current_goal, std::slice::from_ref(&equation))
        )
        .is_ok()
    );
    let entry_goal = goal(entry, IeeeFloatFormat::Binary32);
    assert!(
        accept_certificate(
            &context,
            &entry_goal,
            &[],
            std::slice::from_ref(&equation),
            &certificate(&entry_goal, std::slice::from_ref(&equation))
        )
        .is_err()
    );

    // The exact source value is finite and in f32's range, but loses 2^-30.
    let lossy = goal(
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x3ff8_0000_0040_0000)),
        IeeeFloatFormat::Binary32,
    );
    assert!(
        accept_certificate(
            &PropositionContext::default(),
            &lossy,
            &[],
            &[],
            &certificate(&lossy, &[])
        )
        .is_err()
    );
    let exact = goal(
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x3ff8_0000_0000_0000)),
        IeeeFloatFormat::Binary32,
    );
    assert!(
        accept_certificate(
            &PropositionContext::default(),
            &exact,
            &[],
            &[],
            &certificate(&exact, &[])
        )
        .is_ok()
    );
    let ieee = Proposition::ScalarIeeeFloatComparison {
        kind: semantic_vocabulary::IeeeFloatComparisonKind::Equal,
        format: IeeeFloatFormat::Binary64,
        left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0)),
        right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(1 << 63)),
    };
    assert!(check_float_meaning_normalization(&exact, [&ieee]).is_err());
    let raw = Proposition::Equal(
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0)),
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(1 << 63)),
    );
    assert!(check_float_meaning_normalization(&exact, [&raw]).is_err());
    let different_zeros = literal_equality(
        IeeeFloatValue::Binary64(0),
        IeeeFloatValue::Binary64(1 << 63),
    );
    assert!(
        accept_certificate(
            &PropositionContext::default(),
            &different_zeros,
            &[],
            &[],
            &certificate(&different_zeros, &[])
        )
        .is_err()
    );
    let nan_meanings = literal_equality(
        IeeeFloatValue::Binary64(0x7ff0_0000_0000_0001),
        IeeeFloatValue::Binary64(0xfff8_0000_0000_0003),
    );
    assert!(
        accept_certificate(
            &PropositionContext::default(),
            &nan_meanings,
            &[],
            &[],
            &certificate(&nan_meanings, &[])
        )
        .is_ok()
    );
    let nan_equal = Proposition::ScalarIeeeFloatComparison {
        kind: semantic_vocabulary::IeeeFloatComparisonKind::Equal,
        format: IeeeFloatFormat::Binary64,
        left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x7ff8_0000_0000_0001)),
        right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x7ff8_0000_0000_0001)),
    };
    assert!(
        accept_certificate(
            &PropositionContext::default(),
            &nan_equal,
            &[],
            &[],
            &certificate(&nan_equal, &[])
        )
        .is_err()
    );
}

#[test]
fn normalization_budget_never_publishes_a_partial_success() {
    let wide = value(2, IeeeFloatFormat::Binary64);
    let equation = widening(value(1, IeeeFloatFormat::Binary32), wide.clone());
    let desired = goal(wide, IeeeFloatFormat::Binary32);
    assert_eq!(
        check_float_meaning_normalization(&desired, std::iter::repeat_n(&equation, WORK_LIMIT)),
        Err(FloatMeaningNormalizationError::WorkLimit)
    );
}

#[test]
fn integer_operands_keep_unknown_value_identity_and_exact_carriers() {
    use numerics::integer_carrier::FixedInteger;
    use semantic_vocabulary::IntegerType;

    let signed32 = IntegerType::from_fixed(FixedInteger::signed(32)).unwrap();
    let signed64 = IntegerType::from_fixed(FixedInteger::signed(64)).unwrap();
    let unsigned32 = IntegerType::from_fixed(FixedInteger::unsigned(32)).unwrap();
    let input =
        |raw, carrier| ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Integer(carrier));
    let equation = |source: ScalarTerm, carrier| {
        use FloatSemanticValueKind::{Format, Integer, Meaning};
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[Format, Integer(carrier)],
            Meaning,
        )
        .unwrap();
        Proposition::FloatMeaningEqual(
            FloatMeaningClause::result_equation(
                value(2, IeeeFloatFormat::Binary64),
                operation.contract_identity(),
                IeeeFloatFormat::Binary64,
                [source],
            )
            .unwrap(),
        )
    };
    let premise = equation(input(1, signed32), FixedInteger::signed(32));
    assert!(check_float_meaning_normalization(&premise, [&premise]).is_ok());
    for (different, carrier) in [
        (input(3, signed32), FixedInteger::signed(32)),
        (input(1, signed64), FixedInteger::signed(64)),
        (input(1, unsigned32), FixedInteger::unsigned(32)),
    ] {
        assert_eq!(
            check_float_meaning_normalization(&equation(different, carrier), [&premise]),
            Err(FloatMeaningNormalizationError::NotEstablished),
        );
    }
    let mut forged = premise.clone();
    let Proposition::FloatMeaningEqual(clause) = &mut forged else {
        unreachable!()
    };
    for row in &mut clause.values {
        if let FloatMeaningClauseValue::Application { operands, .. } = row {
            operands[1] = FloatMeaningClauseOperand::Integer(input(1, unsigned32));
        }
    }
    assert_eq!(
        check_float_meaning_normalization(&forged, [&premise]),
        Err(FloatMeaningNormalizationError::Malformed(
            "float meaning application changes its integer carrier"
        )),
    );
}

#[test]
fn integer_literals_use_the_existing_closed_catalog_without_equating_other_values() {
    use FloatSemanticValueKind::{Format, Integer, Meaning};
    use numerics::integer_carrier::FixedInteger;
    use semantic_vocabulary::{IntegerType, IntegerValue};

    let carrier = FixedInteger::signed(32);
    let operation = FloatSemanticOperation::from_source_identity(
        "FloatSemantics",
        "from_integer",
        &[Format, Integer(carrier)],
        Meaning,
    )
    .unwrap();
    let equation = |integer| {
        Proposition::FloatMeaningEqual(
            FloatMeaningClause::result_equation(
                ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(1.0_f64.to_bits())),
                operation.contract_identity(),
                IeeeFloatFormat::Binary64,
                [ScalarTerm::Integer {
                    scalar_type: IntegerType::from_fixed(carrier).unwrap(),
                    value: IntegerValue::Signed(integer),
                }],
            )
            .unwrap(),
        )
    };
    assert!(check_float_meaning_normalization(&equation(1), []).is_ok());
    assert_eq!(
        check_float_meaning_normalization(&equation(2), []),
        Err(FloatMeaningNormalizationError::NotEstablished),
    );
}

#[test]
fn integer_operand_path_exhaustion_refuses_even_a_cited_equation() {
    use FloatSemanticValueKind::{Format, Integer, Meaning};
    use numerics::integer_carrier::FixedInteger;
    use semantic_vocabulary::{CanonicalStructuralPathSegment, IntegerType};

    let carrier = FixedInteger::signed(32);
    let operation = FloatSemanticOperation::from_source_identity(
        "FloatSemantics",
        "from_integer",
        &[Format, Integer(carrier)],
        Meaning,
    )
    .unwrap();
    let clause = FloatMeaningClause::result_equation(
        value(2, IeeeFloatFormat::Binary64),
        operation.contract_identity(),
        IeeeFloatFormat::Binary64,
        [ScalarTerm::integer_field_path(
            PlaceId::new(1).unwrap(),
            vec![CanonicalStructuralPathSegment::FixedIndex(0); WORK_LIMIT],
            IntegerType::from_fixed(carrier).unwrap(),
        )],
    )
    .unwrap();
    clause.validate().unwrap();
    let equation = Proposition::FloatMeaningEqual(clause);
    assert_eq!(
        check_float_meaning_normalization(&equation, [&equation]),
        Err(FloatMeaningNormalizationError::WorkLimit),
    );
}

#[test]
fn normalization_matches_independent_exhaustive_miniature_arithmetic() {
    let narrow = model::NARROW;
    let wide = model::WIDE;
    assert!(narrow.embeds_into(wide));
    let mut count = 0;
    for bits in narrow.encodings() {
        let original = narrow.decode(bits);
        assert_eq!(
            narrow.roundtrip(wide, bits),
            original,
            "mini encoding {bits}"
        );
        if let model::Meaning::Finite(_) = original {
            assert!(
                wide.encodings()
                    .any(|candidate| wide.decode(candidate) == original),
                "embedding must be exact before any rounding"
            );
        }
        let source = ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(
            u32::try_from(narrow.binary_bits(bits, 23, 8, 127)).unwrap(),
        ));
        let wide_value = value(2, IeeeFloatFormat::Binary64);
        let equation = widening(source, wide_value.clone());
        let desired = goal(wide_value, IeeeFloatFormat::Binary32);
        accept_certificate(
            &context(),
            &desired,
            &[],
            std::slice::from_ref(&equation),
            &certificate(&desired, std::slice::from_ref(&equation)),
        )
        .unwrap();
        count += 1;
    }
    assert_eq!(count, 32);
    for deficient in [
        model::Format {
            precision: 2,
            ..wide
        },
        model::Format {
            precision: 4,
            exponent_bits: 2,
            bias: 2,
        },
        model::Format {
            precision: 4,
            exponent_bits: 2,
            bias: -1,
        },
    ] {
        assert!(!narrow.embeds_into(deficient));
        assert!(
            narrow
                .encodings()
                .any(|bits| narrow.roundtrip(deficient, bits) != narrow.decode(bits)),
            "each omitted precision/exponent/quantum condition has a computed counterexample"
        );
    }
    assert!(!IeeeFloatFormat::Binary64.can_exactly_widen_to(IeeeFloatFormat::Binary32));
}
