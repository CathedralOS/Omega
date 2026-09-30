use super::super::{BTreeSet, CheckedIntegerComparisonKind, value_id};
use super::{
    BTreeMap, CheckedBooleanExpression, CheckedScalarExpression, LoweringError, PrimitiveType,
    Proposition, ScalarTerm, ScalarType, ValueDeclaration, integer_scalar_type,
    lower_structural_runtime_requirement, substitute_runtime_requirement_scalar_values,
};
use numerics::arithmetic::ArithmeticDomain;

fn boolean_parameter() -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(1),
        scalar_type: ScalarType::Boolean,
    }
}

fn lower(expression: &CheckedBooleanExpression) -> Result<Proposition, LoweringError> {
    lower_structural_runtime_requirement(expression, &[boolean_parameter()], &[], &[], &[])
}

#[test]
fn total_integer_requirements_preserve_arithmetic_and_substitute_nested_formals() {
    let scalar_type = integer_scalar_type(PrimitiveType::U64).unwrap();
    let formal = ValueDeclaration {
        id: value_id(1),
        scalar_type,
        qualifications: Default::default(),
    };
    let parameter = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    let literal = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(1).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U64,
                domain: ArithmeticDomain::Exact,
            },
        ),
    };
    let expression = CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::LessThan,
        left: Box::new(CheckedScalarExpression::IntegerBinary {
            kind: checked_trees::CheckedIntegerBinaryKind::WrappingSubtract,
            primitive_type: PrimitiveType::U64,
            left: Box::new(parameter.clone()),
            right: Box::new(literal),
        }),
        right: Box::new(parameter),
    };
    let mut proposition =
        lower_structural_runtime_requirement(&expression, &[formal], &[], &[], &[])
            .expect("total arithmetic belongs to the entry predicate, not a new body shape");
    assert!(matches!(
        &proposition,
        Proposition::LessThan(ScalarTerm::WrappingIntegerSubtract { .. }, _)
    ));
    let actual = ValueDeclaration {
        id: value_id(8),
        ..formal
    };
    substitute_runtime_requirement_scalar_values(
        &mut proposition,
        &BTreeMap::from([(formal.id, actual)]),
    )
    .expect("substitute every nested formal without changing the operation");
    let mut values = BTreeSet::new();
    proposition.visit_value_ids(|value| {
        values.insert(value);
    });
    assert_eq!(values, BTreeSet::from([actual.id]));
    for invalid in [
        CheckedScalarExpression::StorageRead {
            symbol: symbols::SymbolHandle::from_arena_index(9),
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U32,
        },
    ] {
        let mut forged = expression.clone();
        let CheckedBooleanExpression::IntegerComparison { left, .. } = &mut forged else {
            panic!("integer requirement");
        };
        let CheckedScalarExpression::IntegerBinary { left, .. } = left.as_mut() else {
            panic!("arithmetic operand");
        };
        *left = Box::new(invalid);
        assert!(
            lower_structural_runtime_requirement(&forged, &[formal], &[], &[], &[]).is_err(),
            "arithmetic cannot hide a mutable read or a mismatched formal carrier"
        );
    }
}

#[test]
fn wrappers_never_admit_body_locals_or_mutable_storage() {
    for unsupported in [
        CheckedBooleanExpression::Local { position: 0 },
        CheckedBooleanExpression::StorageRead {
            symbol: symbols::SymbolHandle::from_arena_index(9),
        },
    ] {
        for expression in [
            unsupported.clone(),
            CheckedBooleanExpression::Not(Box::new(unsupported.clone())),
            CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Constant(true)),
                right: Box::new(unsupported.clone()),
            },
            CheckedBooleanExpression::Equal {
                left: Box::new(CheckedBooleanExpression::Constant(false)),
                right: Box::new(unsupported),
            },
        ] {
            assert!(lower(&expression).is_err());
        }
    }
}

#[test]
fn boolean_parameters_require_exact_namespace_and_actual_carriers() {
    let expression = CheckedBooleanExpression::Parameter { position: 0 };
    let proposition = lower(&expression).unwrap();
    let wrong_type = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(2),
        scalar_type: integer_scalar_type(PrimitiveType::U8).unwrap(),
    };
    assert!(
        lower_structural_runtime_requirement(&expression, &[wrong_type], &[], &[], &[]).is_err()
    );
    assert!(lower(&CheckedBooleanExpression::Parameter { position: 1 }).is_err());
    for substitutions in [BTreeMap::new(), BTreeMap::from([(value_id(1), wrong_type)])] {
        assert!(
            substitute_runtime_requirement_scalar_values(&mut proposition.clone(), &substitutions)
                .is_err()
        );
    }
    let actual = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(8),
        scalar_type: ScalarType::Boolean,
    };
    let mut rebound = proposition;
    substitute_runtime_requirement_scalar_values(
        &mut rebound,
        &BTreeMap::from([(value_id(1), actual)]),
    )
    .unwrap();
    assert!(matches!(rebound, Proposition::Equal(left, right)
        if left == ScalarTerm::value(actual.id, actual.scalar_type)
            && right == ScalarTerm::boolean(true)
            || right == ScalarTerm::value(actual.id, actual.scalar_type)
                && left == ScalarTerm::boolean(true)));
}

#[test]
fn compound_boolean_entry_equality_keeps_substitutable_scalar_denotations() {
    use CheckedBooleanExpression as Boolean;
    let first = Boolean::Parameter { position: 0 };
    let second = Boolean::Parameter { position: 1 };
    let parameters = [
        boolean_parameter(),
        ValueDeclaration {
            id: value_id(2),
            ..boolean_parameter()
        },
    ];
    for right in [
        Boolean::Not(Box::new(second.clone())),
        Boolean::Equal {
            left: Box::new(first.clone()),
            right: Box::new(second),
        },
    ] {
        let expression = Boolean::Equal {
            left: Box::new(first.clone()),
            right: Box::new(right),
        };
        let mut proposition =
            lower_structural_runtime_requirement(&expression, &parameters, &[], &[], &[]).unwrap();
        assert!(matches!(proposition, Proposition::Equal(_, _)));
        substitute_runtime_requirement_scalar_values(
            &mut proposition,
            &BTreeMap::from([
                (
                    value_id(1),
                    ValueDeclaration {
                        id: value_id(9),
                        ..boolean_parameter()
                    },
                ),
                (
                    value_id(2),
                    ValueDeclaration {
                        id: value_id(8),
                        ..boolean_parameter()
                    },
                ),
            ]),
        )
        .unwrap();
        let mut values = BTreeSet::new();
        proposition.visit_value_ids(|value| {
            values.insert(value);
        });
        assert_eq!(values, BTreeSet::from([value_id(8), value_id(9)]));
    }
}

#[test]
fn strict_integer_requirements_keep_original_relation_and_reject_new_arithmetic() {
    let scalar_type = integer_scalar_type(PrimitiveType::U16).unwrap();
    let formal = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(1),
        scalar_type,
    };
    let parameter = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let literal = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(3).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U16,
                domain: ArithmeticDomain::Exact,
            },
        ),
    };
    let predicate = |left| CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::LessThan,
        left: Box::new(left),
        right: Box::new(literal.clone()),
    };
    assert!(matches!(
        lower_structural_runtime_requirement(
            &predicate(parameter.clone()),
            &[formal],
            &[],
            &[],
            &[]
        )
        .unwrap(),
        Proposition::LessThan(_, _)
    ));
    for kind in [
        checked_trees::CheckedIntegerBinaryKind::ExactAdd,
        checked_trees::CheckedIntegerBinaryKind::ExactDivide,
    ] {
        let arithmetic = CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type: PrimitiveType::U16,
            left: Box::new(parameter.clone()),
            right: Box::new(literal.clone()),
        };
        assert!(
            lower_structural_runtime_requirement(&predicate(arithmetic), &[formal], &[], &[], &[])
                .is_err()
        );
    }
    for primitive_type in [PrimitiveType::Addr, PrimitiveType::F32] {
        assert!(
            lower_structural_runtime_requirement(
                &predicate(CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type,
                }),
                &[formal],
                &[],
                &[],
                &[]
            )
            .is_err()
        );
    }
}

#[test]
fn mixed_integer_requirements_substitute_each_exact_formal_under_both_polarities() {
    let formals = [PrimitiveType::I32, PrimitiveType::U64].map(|primitive_type| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(if primitive_type == PrimitiveType::I32 {
            1
        } else {
            2
        }),
        scalar_type: integer_scalar_type(primitive_type).unwrap(),
    });
    let actuals = [
        ValueDeclaration {
            id: value_id(9),
            ..formals[0]
        },
        ValueDeclaration {
            id: value_id(8),
            ..formals[1]
        },
    ];
    let substitutions = BTreeMap::from([(formals[0].id, actuals[0]), (formals[1].id, actuals[1])]);
    for kind in [
        CheckedIntegerComparisonKind::Equal,
        CheckedIntegerComparisonKind::LessThan,
        CheckedIntegerComparisonKind::LessOrEqual,
    ] {
        for negative in [false, true] {
            let comparison = CheckedBooleanExpression::IntegerComparison {
                kind,
                left: Box::new(CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: PrimitiveType::I32,
                }),
                right: Box::new(CheckedScalarExpression::Parameter {
                    position: 1,
                    primitive_type: PrimitiveType::U64,
                }),
            };
            let predicate = if negative {
                CheckedBooleanExpression::Not(Box::new(comparison))
            } else {
                comparison
            };
            let original =
                lower_structural_runtime_requirement(&predicate, &formals, &[], &[], &[]).unwrap();
            let expected =
                lower_structural_runtime_requirement(&predicate, &actuals, &[], &[], &[]).unwrap();
            let mut rebound = original.clone();
            substitute_runtime_requirement_scalar_values(&mut rebound, &substitutions).unwrap();
            assert_eq!(rebound, expected, "{kind:?}, negative={negative}");
            for formal in formals {
                let mut missing = substitutions.clone();
                missing.remove(&formal.id);
                assert!(
                    substitute_runtime_requirement_scalar_values(&mut original.clone(), &missing)
                        .is_err()
                );
                for wrong_type in [
                    ScalarType::Boolean,
                    integer_scalar_type(PrimitiveType::U32).unwrap(),
                ] {
                    let mut forged = substitutions.clone();
                    forged.get_mut(&formal.id).unwrap().scalar_type = wrong_type;
                    assert!(
                        substitute_runtime_requirement_scalar_values(
                            &mut original.clone(),
                            &forged
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn mixed_comparison_substitution_retains_structural_endpoint_and_rejects_forged_witness() {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerSign, IntegerType, PlaceId, StructuralFieldId,
    };
    let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let field = ScalarTerm::integer_field_path(
        PlaceId::new(1).unwrap(),
        vec![CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(2).unwrap(),
        )],
        signed,
    );
    let formal = ValueDeclaration {
        id: value_id(1),
        qualifications: Default::default(),
        scalar_type: ScalarType::Integer(unsigned),
    };
    let actual = ValueDeclaration {
        id: value_id(8),
        ..formal
    };
    let predicate = |witness, value| {
        Proposition::Equal(
            ScalarTerm::IntegerLessThan {
                scalar_type: witness,
                left: Box::new(field.clone()),
                right: Box::new(value),
            },
            ScalarTerm::Boolean(true),
        )
    };
    let substitutions = BTreeMap::from([(formal.id, actual)]);
    let mut requirement = predicate(signed, ScalarTerm::value(formal.id, formal.scalar_type));
    substitute_runtime_requirement_scalar_values(&mut requirement, &substitutions).unwrap();
    assert_eq!(
        requirement,
        predicate(signed, ScalarTerm::value(actual.id, actual.scalar_type))
    );
    let mut forged = predicate(unsigned, ScalarTerm::value(formal.id, formal.scalar_type));
    assert!(substitute_runtime_requirement_scalar_values(&mut forged, &substitutions).is_err());
}
