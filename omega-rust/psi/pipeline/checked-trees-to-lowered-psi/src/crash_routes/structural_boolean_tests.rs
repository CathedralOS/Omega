//! Owner tests for structural crash predicates, not fabricated source evidence.

use super::*;
use semantic_vocabulary::PsiSemanticId;

fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
    Identity::new(raw).unwrap()
}

fn path(name: &str) -> Vec<checked_trees::CheckedStructuralPredicatePathSegment> {
    vec![checked_trees::CheckedStructuralPredicatePathSegment::Field(
        name.into(),
    )]
}

fn field(name: &str) -> CheckedBooleanExpression {
    CheckedBooleanExpression::StructuralParameterField {
        parameter_position: 2,
        path: path(name),
    }
}

fn and(
    left: CheckedBooleanExpression,
    right: CheckedBooleanExpression,
) -> CheckedBooleanExpression {
    CheckedBooleanExpression::And {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn equal(
    left: CheckedBooleanExpression,
    right: CheckedBooleanExpression,
) -> CheckedBooleanExpression {
    CheckedBooleanExpression::Equal {
        left: Box::new(left),
        right: Box::new(right),
    }
}

struct Fixture {
    scalars: Vec<ValueDeclaration>,
    parameters: Vec<StructuralParameterDeclaration>,
    types: Vec<StructuralTypeDeclaration>,
}

impl Fixture {
    fn new() -> Self {
        let fields = [
            ("flag", StructuralFieldType::Scalar(ScalarType::Boolean)),
            ("other", StructuralFieldType::Scalar(ScalarType::Boolean)),
            (
                "count",
                StructuralFieldType::Scalar(integer_scalar_type(PrimitiveType::I32).unwrap()),
            ),
            (
                "first",
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            ),
            (
                "second",
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, field_type))| StructuralFieldDeclaration {
            id: id(index as u64 + 1),
            identity: name.into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type,
        })
        .collect();
        Self {
            scalars: vec![ValueDeclaration {
                id: id(7),
                scalar_type: ScalarType::Boolean,
            }],
            parameters: vec![StructuralParameterDeclaration {
                place: id(11),
                position: 2,
                is_self: false,
                structural_type: id(3),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            types: vec![StructuralTypeDeclaration {
                id: id(3),
                identity: "test::Record".into(),
                shape: StructuralTypeShape::Record { fields },
            }],
        }
    }

    fn lower(&self, expression: CheckedBooleanExpression) -> Result<Proposition, LoweringError> {
        // This unit owns checked-to-terminal conversion only. The opaque
        // authored identity does not claim compilation or admission authority.
        let identity = checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
            checked_trees::CrashPredicateExpression::Parameter(0),
            expression,
        );
        let bucket = checked_trees::CrashRouteBucket::new(
            checked_trees::CrashCause::Trap,
            vec![checked_trees::CrashRouteGuard::Predicate(identity)],
        )
        .unwrap();
        let lowered = lower_structural_crash_route_buckets(
            &[bucket],
            &self.scalars,
            &self.parameters,
            &self.types,
            &[],
        )?;
        assert_eq!(lowered.len(), 1);
        assert_eq!(lowered[0].cause, TerminalCrashCause::Trap);
        let [terminal_psi::CrashRouteGuard::Predicate(predicate)] =
            lowered[0].alternatives.as_slice()
        else {
            panic!("one checked predicate remains one published alternative")
        };
        Ok(predicate.proposition().clone())
    }
}

fn holds(term: ScalarTerm) -> Proposition {
    let mut terms = [term, ScalarTerm::boolean(true)];
    terms.sort_by_key(|term| terminal_codec::canonical_scalar_term_order_key(term).unwrap());
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

fn logical(mut children: Vec<Proposition>, conjunction: bool) -> Proposition {
    children.sort_by_key(|child| terminal_codec::canonical_proposition_order_key(child).unwrap());
    children.dedup();
    if children.len() == 1 {
        return children.pop().unwrap();
    }
    if conjunction {
        Proposition::Conjunction(children)
    } else {
        Proposition::Disjunction(children)
    }
}

fn assert_bytes(actual: Proposition, expected: Proposition) {
    assert_eq!(actual, expected);
    assert_eq!(
        terminal_codec::canonical_proposition_order_key(&actual).unwrap(),
        terminal_codec::canonical_proposition_order_key(&expected).unwrap(),
    );
}

#[test]
fn atomic_boolean_integer_and_constant_connective_encodings_remain_exact() {
    let fixture = Fixture::new();
    let flag = ScalarTerm::boolean_field(id(11), id(1));
    assert_bytes(fixture.lower(field("flag")).unwrap(), holds(flag.clone()));
    assert_bytes(
        fixture
            .lower(CheckedBooleanExpression::Not(Box::new(field("flag"))))
            .unwrap(),
        holds(ScalarTerm::boolean_not(flag.clone()).unwrap()),
    );
    let integer = match integer_scalar_type(PrimitiveType::I32).unwrap() {
        ScalarType::Integer(integer) => integer,
        _ => unreachable!(),
    };
    let count = CheckedScalarExpression::StructuralParameterField {
        parameter_position: 2,
        path: path("count"),
        primitive_type: PrimitiveType::I32,
    };
    let compared = CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::Equal,
        left: Box::new(count.clone()),
        right: Box::new(count),
    };
    let count = ScalarTerm::integer_field_path(
        id(11),
        vec![CanonicalStructuralPathSegment::Field(id(3))],
        integer,
    );
    assert_bytes(
        fixture.lower(compared).unwrap(),
        holds(ScalarTerm::integer_equal(integer, count.clone(), count).unwrap()),
    );
    assert_bytes(
        fixture
            .lower(and(field("flag"), CheckedBooleanExpression::Constant(true)))
            .unwrap(),
        logical(vec![holds(flag), holds(ScalarTerm::boolean(true))], true),
    );
}

#[test]
fn compound_field_equality_retains_exact_scalar_and_structural_namespaces() {
    let fixture = Fixture::new();
    let flag = ScalarTerm::boolean_field(id(11), id(1));
    let other = ScalarTerm::boolean_field(id(11), id(2));
    let scalar = ScalarTerm::value(id(7), ScalarType::Boolean);
    let expected = logical(
        vec![
            logical(
                vec![
                    holds(flag.clone()),
                    holds(other.clone()),
                    holds(scalar.clone()),
                ],
                true,
            ),
            logical(
                vec![
                    logical(
                        vec![
                            holds(ScalarTerm::boolean_not(flag).unwrap()),
                            holds(ScalarTerm::boolean_not(other).unwrap()),
                        ],
                        false,
                    ),
                    holds(ScalarTerm::boolean_not(scalar).unwrap()),
                ],
                true,
            ),
        ],
        false,
    );
    assert_bytes(
        fixture
            .lower(equal(
                and(field("flag"), field("other")),
                CheckedBooleanExpression::Parameter { position: 0 },
            ))
            .unwrap(),
        expected,
    );
}

#[test]
fn special_atomic_negation_keeps_implication_to_falsehood() {
    let fixture = Fixture::new();
    let atom = CheckedBooleanExpression::IeeeFloatComparison {
        kind: checked_trees::CheckedIeeeFloatComparisonKind::Equal,
        primitive_type: PrimitiveType::F32,
        left: checked_trees::CheckedStructuralParameterField {
            parameter_position: 2,
            path: path("first"),
        },
        right: checked_trees::CheckedStructuralParameterField {
            parameter_position: 2,
            path: path("second"),
        },
    };
    let expected = Proposition::IeeeFloatComparison {
        kind: semantic_vocabulary::IeeeFloatComparisonKind::Equal,
        format: IeeeFloatFormat::Binary32,
        left: IeeeFloatStructuralField::new(
            id(11),
            vec![CanonicalStructuralPathSegment::Field(id(4))],
        )
        .unwrap(),
        right: IeeeFloatStructuralField::new(
            id(11),
            vec![CanonicalStructuralPathSegment::Field(id(5))],
        )
        .unwrap(),
    };
    assert_bytes(fixture.lower(atom.clone()).unwrap(), expected.clone());
    assert_bytes(
        fixture
            .lower(CheckedBooleanExpression::Not(Box::new(atom)))
            .unwrap(),
        Proposition::Implication {
            premise: Box::new(expected),
            conclusion: Box::new(Proposition::Falsehood),
        },
    );
}

#[test]
fn malformed_structural_and_scalar_operands_do_not_gain_equality_meaning() {
    let fixture = Fixture::new();
    for wrong in [
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 0,
            path: path("flag"),
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 2,
            path: path("absent"),
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 2,
            path: Vec::new(),
        },
        field("count"),
        CheckedBooleanExpression::Parameter { position: 1 },
        CheckedBooleanExpression::StorageRead {
            symbol: symbols::SymbolHandle::invalid(),
        },
    ] {
        assert!(
            fixture
                .lower(equal(and(field("flag"), field("other")), wrong))
                .is_err()
        );
    }
    let mut wrong_type = Fixture::new();
    wrong_type.parameters[0].structural_type = id(99);
    assert!(wrong_type.lower(field("flag")).is_err());
    let mut wrong_scalar = Fixture::new();
    wrong_scalar.scalars[0].scalar_type = integer_scalar_type(PrimitiveType::I32).unwrap();
    assert!(
        wrong_scalar
            .lower(equal(
                and(field("flag"), field("other")),
                CheckedBooleanExpression::Parameter { position: 0 }
            ))
            .is_err()
    );
}

#[test]
fn structural_composition_has_one_depth_and_expansion_budget() {
    let fixture = Fixture::new();
    let mut deep = and(field("flag"), field("other"));
    for _ in 0..64 {
        deep = CheckedBooleanExpression::Not(Box::new(deep));
    }
    assert!(
        matches!(fixture.lower(deep), Err(LoweringError::Unsupported(message)) if message.contains("depth limit"))
    );
    let mut expensive = and(field("flag"), field("other"));
    for _ in 0..12 {
        expensive = equal(
            expensive,
            CheckedBooleanExpression::Parameter { position: 0 },
        );
    }
    assert!(
        matches!(fixture.lower(expensive), Err(LoweringError::Unsupported(message)) if message.contains("lowering budget"))
    );
}
