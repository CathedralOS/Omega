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

fn float_field_fixture(
    primitive_type: PrimitiveType,
) -> (
    terminal_psi::StructuralParameterDeclaration,
    Vec<terminal_psi::StructuralTypeDeclaration>,
    CheckedScalarExpression,
) {
    use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape,
    };
    let scalar_type = super::terminal_scalar_type(primitive_type).unwrap();
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(17).unwrap(),
        position: 3,
        is_self: false,
        structural_type: StructuralTypeId::new(10).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let types = [
        (
            10,
            "child",
            StructuralFieldType::Structural(StructuralTypeId::new(11).unwrap()),
        ),
        (11, "sample", StructuralFieldType::Scalar(scalar_type)),
    ]
    .into_iter()
    .map(|(identity, name, field_type)| StructuralTypeDeclaration {
        id: StructuralTypeId::new(identity).unwrap(),
        identity: format!("test::FloatRecord{identity}"),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(identity).unwrap(),
                identity: name.into(),
                relevance: BindingRelevance::Relevant,
                field_type,
            }],
        },
    })
    .collect();
    let expression = CheckedScalarExpression::StructuralParameterField {
        parameter_position: 3,
        path: ["child", "sample"]
            .into_iter()
            .map(|name| checked_trees::CheckedStructuralPredicatePathSegment::Field(name.into()))
            .collect(),
        primitive_type,
    };
    (parameter, types, expression)
}

#[test]
fn ieee_entry_fields_keep_exact_paths_formats_and_unordered_negation() {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IeeeFloatComparisonKind as Kind, IeeeFloatValue,
        StructuralFieldId,
    };
    for (primitive_type, bits) in [
        (PrimitiveType::F32, IeeeFloatValue::Binary32(0x3f80_0000)),
        (
            PrimitiveType::F64,
            IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
        ),
    ] {
        let (parameter, types, field) = float_field_fixture(primitive_type);
        let comparison = CheckedBooleanExpression::ScalarIeeeFloatComparison {
            kind: Kind::LessOrEqual,
            left: Box::new(field),
            right: Box::new(CheckedScalarExpression::IeeeFloatLiteral { value: bits }),
        };
        for negative in [false, true] {
            let expression = if negative {
                CheckedBooleanExpression::Not(Box::new(comparison.clone()))
            } else {
                comparison.clone()
            };
            let mut proposition = lower_structural_runtime_requirement(
                &expression,
                &[],
                &[],
                &[parameter.clone()],
                &types,
            )
            .unwrap();
            assert_eq!(
                proposition,
                Proposition::ScalarIeeeFloatComparison {
                    kind: if negative {
                        Kind::NotLessOrEqual
                    } else {
                        Kind::LessOrEqual
                    },
                    format: bits.format(),
                    left: ScalarTerm::ieee_float_field_path(
                        parameter.place,
                        [10, 11]
                            .map(|identity| CanonicalStructuralPathSegment::Field(
                                StructuralFieldId::new(identity).unwrap()
                            ))
                            .to_vec(),
                        bits.format()
                    ),
                    right: ScalarTerm::IeeeFloatLiteral(bits),
                }
            );
            let original = proposition.clone();
            substitute_runtime_requirement_scalar_values(&mut proposition, &BTreeMap::new())
                .unwrap();
            assert_eq!(
                proposition, original,
                "scalar substitutions do not rename storage"
            );
        }
    }
}

#[test]
fn ieee_entry_fields_reject_wrong_format_parameter_and_member_paths() {
    use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatValue};
    let (parameter, types, field) = float_field_fixture(PrimitiveType::F32);
    for mutation in 0..5 {
        let mut field = field.clone();
        let CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } = &mut field
        else {
            unreachable!()
        };
        match mutation {
            0 => *primitive_type = PrimitiveType::F64,
            1 => *parameter_position = 4,
            2 => path.clear(),
            3 => {
                path.pop();
            }
            4 => {
                path[1] =
                    checked_trees::CheckedStructuralPredicatePathSegment::Field("missing".into())
            }
            _ => unreachable!(),
        }
        let expression = CheckedBooleanExpression::ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind::LessOrEqual,
            left: Box::new(field),
            right: Box::new(CheckedScalarExpression::IeeeFloatLiteral {
                value: IeeeFloatValue::Binary32(0x3f80_0000),
            }),
        };
        assert!(
            lower_structural_runtime_requirement(
                &expression,
                &[],
                &[],
                &[parameter.clone()],
                &types
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ieee_entry_comparisons_reject_hidden_storage_reads_and_mixed_formats() {
    use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatValue};
    let literal = CheckedScalarExpression::IeeeFloatLiteral {
        value: IeeeFloatValue::Binary32(0x7fc0_1234),
    };
    let predicate = |left| CheckedBooleanExpression::ScalarIeeeFloatComparison {
        kind: IeeeFloatComparisonKind::NotEqual,
        left: Box::new(left),
        right: Box::new(literal.clone()),
    };
    let closed = lower(&predicate(literal.clone())).unwrap();
    assert!(
        matches!(
            closed,
            Proposition::ScalarIeeeFloatComparison {
                kind: IeeeFloatComparisonKind::NotEqual,
                ..
            }
        ),
        "NaN comparison retains IEEE meaning"
    );
    for operand in [
        CheckedScalarExpression::IeeeFloatLiteral {
            value: IeeeFloatValue::Binary64(0),
        },
        CheckedScalarExpression::StorageRead {
            symbol: symbols::SymbolHandle::from_arena_index(9),
            primitive_type: PrimitiveType::F32,
        },
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U32,
        },
    ] {
        let comparison = predicate(operand);
        for expression in [
            comparison.clone(),
            CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Constant(true)),
                right: Box::new(comparison),
            },
        ] {
            assert!(
                lower(&expression).is_err(),
                "simplification must not hide an invalid IEEE operand"
            );
        }
    }
}
