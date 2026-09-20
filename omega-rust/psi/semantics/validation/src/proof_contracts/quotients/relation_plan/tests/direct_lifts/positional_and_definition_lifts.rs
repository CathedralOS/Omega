use crate::proof_contracts::quotients::relation_plan::tests::{
    boolean_array_literal, boolean_tensor3_literal, bounded_byte_buffer_type,
    byte_slice_reference_type, call_with_arguments, canonical_byte_array_literal, carrier_type,
    fixed_boolean_array_type, fixed_boolean_tensor3_type, fixed_byte_array_type,
    fixed_float_array_type, fixed_integer_array_type, fixed_nested_boolean_array_type,
    fixed_nested_byte_array_type, fixed_nested_float_array_type, fixed_nested_integer_array_type,
    float_array_literal, integer_array_literal, named_argument, nested_boolean_array_literal,
    nested_canonical_byte_array_literal, nested_float_array_literal, nested_integer_array_literal,
    primitive_type, push_representative, quotient_type, symbol, wrap_array_literal,
    wrap_fixed_array_type,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema::derive_expected_theorem_schema;
use crate::proof_contracts::quotients::relation_plan::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeRuntimeParameter,
    RepresentativeStaticApplication, RepresentativeTelescope, derive_define_runtime_correspondence,
    derive_direct_lift_runtime_correspondence, derive_direct_terminal_plan,
};
use arena::HandleSpan;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{
    FloatFormat, FloatLiteral, IntegerLanding, IntegerLiteral, LandedIntegerType,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionNode, QuotientOperationKind, TableBinaryExpression,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

#[test]
fn direct_lift_runtime_accepts_exact_nested_integer_array_literals() {
    use crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedIntegerArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(990),
        "NestedIntegerArrayLiteralQ",
        symbol(991),
        "NestedIntegerArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_integers = fixed_nested_integer_array_type(&mut program, "i8", 2, 2);
    let i8_landing = IntegerLanding {
        landed_type: LandedIntegerType::I8,
        domain: ArithmeticDomain::Exact,
    };
    let literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![
                IntegerLiteral::from_value(-1),
                IntegerLiteral::from_value(127).with_landing(i8_landing),
            ],
            vec![IntegerLiteral::from_value(3), IntegerLiteral::from_value(2)],
        ],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_integers, false, false)], carrier);

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("every integer-matrix leaf follows the exact scalar landing rule");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_integers)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested integer-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
            source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedIntegerArray {
                    rows: Arc::from(vec![
                        Arc::from(vec![
                            ClosedIntegerArrayElement {
                                spelling: "-1".to_owned(),
                                landing: i8_landing,
                            },
                            ClosedIntegerArrayElement {
                                spelling: "127".to_owned(),
                                landing: i8_landing,
                            },
                        ]),
                        Arc::from(vec![
                            ClosedIntegerArrayElement {
                                spelling: "3".to_owned(),
                                landing: i8_landing,
                            },
                            ClosedIntegerArrayElement {
                                spelling: "2".to_owned(),
                                landing: i8_landing,
                            },
                        ]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_integers),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut evidence_drift = runtime.clone();
    evidence_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedIntegerArray {
            rows: Arc::from(vec![
                Arc::from(vec![ClosedIntegerArrayElement {
                    spelling: "-1".to_owned(),
                    landing: i8_landing,
                }]),
                Arc::from(vec![
                    ClosedIntegerArrayElement {
                        spelling: "127".to_owned(),
                        landing: i8_landing,
                    },
                    ClosedIntegerArrayElement {
                        spelling: "3".to_owned(),
                        landing: i8_landing,
                    },
                    ClosedIntegerArrayElement {
                        spelling: "2".to_owned(),
                        landing: IntegerLanding {
                            landed_type: LandedIntegerType::I8,
                            domain: ArithmeticDomain::Wrapping,
                        },
                    },
                ]),
            ]),
            target_type: program.normalized_type_identity(fixed_integers),
        },
    );
    assert_ne!(
        runtime, evidence_drift,
        "row boundaries and integer landing domains remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_float_array_literals() {
    use crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedFloatArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(980),
        "FloatArrayLiteralQ",
        symbol(981),
        "FloatArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let f32_array = fixed_float_array_type(&mut program, "f32", 2);
    let f64_array = fixed_float_array_type(&mut program, "f64", 1);
    let f32_literal = float_array_literal(
        &mut program,
        [
            FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
            FloatLiteral::parse("2.5").expect("anonymous exact decimal literal"),
        ],
    );
    let f64_literal = float_array_literal(
        &mut program,
        [FloatLiteral::parse("3.75f64").expect("format-landed f64 literal")],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([f32_literal, f64_literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(f32_array, false, false), (f64_array, false, false)],
        carrier,
    );

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("each fixed float-array element follows the exact scalar format rule");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(f32_array),
            InputRelation::ExactEquality(f64_array),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("float-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::FloatArray {
                        elements: Arc::from(vec![
                            ClosedFloatArrayElement {
                                spelling: "1.25".to_owned(),
                                landing: FloatFormat::F32,
                            },
                            ClosedFloatArrayElement {
                                spelling: "2.5".to_owned(),
                                landing: FloatFormat::F32,
                            },
                        ]),
                        target_type: program.normalized_type_identity(f32_array),
                    },
                ),
                representative_parameter: symbol(100),
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::FloatArray {
                        elements: Arc::from(vec![ClosedFloatArrayElement {
                            spelling: "3.75".to_owned(),
                            landing: FloatFormat::F64,
                        }]),
                        target_type: program.normalized_type_identity(f64_array),
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    let mut format_drift = runtime.clone();
    format_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::FloatArray {
            elements: Arc::from(vec![
                ClosedFloatArrayElement {
                    spelling: "1.25".to_owned(),
                    landing: FloatFormat::F64,
                },
                ClosedFloatArrayElement {
                    spelling: "2.5".to_owned(),
                    landing: FloatFormat::F32,
                },
            ]),
            target_type: program.normalized_type_identity(f32_array),
        },
    );
    assert_ne!(runtime, format_drift, "element format remains identity");
}

#[test]
fn direct_lift_runtime_accepts_exact_nested_float_array_literals() {
    use crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedFloatArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(994),
        "NestedFloatArrayLiteralQ",
        symbol(995),
        "NestedFloatArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_floats = fixed_nested_float_array_type(&mut program, "f32", 2, 2);
    let literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![
                FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
                FloatLiteral::parse("2.5f32").expect("format-landed f32 literal"),
            ],
            vec![
                FloatLiteral::parse("3.75").expect("anonymous exact decimal literal"),
                FloatLiteral::parse("4.5f32").expect("format-landed f32 literal"),
            ],
        ],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_floats, false, false)], carrier);

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("every float-matrix leaf follows the exact scalar format rule");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_floats)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested float-array literal runtime correspondence");
    let element = |spelling: &str, landing| ClosedFloatArrayElement {
        spelling: spelling.to_owned(),
        landing,
    };
    assert_eq!(
        runtime.positions,
        [crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
            source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedFloatArray {
                    rows: Arc::from(vec![
                        Arc::from(vec![
                            element("1.25", FloatFormat::F32),
                            element("2.5", FloatFormat::F32),
                        ]),
                        Arc::from(vec![
                            element("3.75", FloatFormat::F32),
                            element("4.5", FloatFormat::F32),
                        ]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_floats),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut evidence_drift = runtime.clone();
    evidence_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedFloatArray {
            rows: Arc::from(vec![
                Arc::from(vec![element("1.25", FloatFormat::F32)]),
                Arc::from(vec![
                    element("2.5", FloatFormat::F32),
                    element("3.75", FloatFormat::F64),
                    element("4.5", FloatFormat::F32),
                ]),
            ]),
            target_type: program.normalized_type_identity(fixed_floats),
        },
    );
    assert_ne!(
        runtime, evidence_drift,
        "row boundaries and float formats remain evidence identity"
    );
}

#[test]
fn repeated_equal_direct_lift_literals_keep_distinct_runtime_and_theorem_positions() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(960),
        "LiteralQ",
        symbol(961),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let i32_type = primitive_type(&mut program, "i32");
    let literal = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I32,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let arguments = program
        .expression_table
        .insert_expression_handles([literal, literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(962),
        state_symbol: symbol(963),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(964),
                type_reference: i32_type,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(965),
                type_reference: i32_type,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: quotient,
        quotient_symbol: symbol(960),
        relation_symbol: symbol(961),
    };
    let input_relations = [
        InputRelation::ExactEquality(i32_type),
        InputRelation::ExactEquality(i32_type),
    ];
    let runtime = derive_direct_lift_runtime_correspondence(
        &program,
        &Machine::default(),
        &state,
        &call,
        &input_relations,
        relation,
        &representative,
    )
    .expect("equal literals may repeat without coalescing their positions");
    assert_eq!(runtime.positions.len(), 2);
    assert_eq!(runtime.positions[0].source, runtime.positions[1].source);
    assert_ne!(
        runtime.positions[0].representative_parameter,
        runtime.positions[1].representative_parameter,
    );
    let expected =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("equal literals remain distinct universal theorem positions");
    assert_eq!(expected.parameters.len(), 2);
    assert_eq!(expected.left_application.arguments, [0, 1]);
    assert_eq!(expected.right_application.arguments, [0, 1]);
    assert!(
        expected.relation_premises.is_empty(),
        "ordinary exact-equality positions share their binders instead of adding quotient premises"
    );
}

#[test]
fn direct_lift_literal_fences_mismatched_and_unadmitted_values() {
    let mut program = TypedTrees::default();
    let i8_type = primitive_type(&mut program, "i8");
    let u8_type = primitive_type(&mut program, "u8");
    let bool_type = primitive_type(&mut program, "bool");
    let f32_type = primitive_type(&mut program, "f32");
    let f64_type = primitive_type(&mut program, "f64");
    let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let cases = [
        (
            program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_value(7).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::I16,
                    domain: ArithmeticDomain::Exact,
                }),
            )),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(0),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(128))),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(1),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(-1))),
            u8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(2),
        ),
        (
            program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_value(7).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::I8,
                    domain: ArithmeticDomain::Wrapping,
                }),
            )),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(3),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Boolean(true)),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(4),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Boolean(true)),
            unit_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(5),
        ),
    ];
    for (position, (expression, target, expected)) in cases.iter().copied().enumerate() {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position,),
            Err(expected),
        );
    }
    let landed_float = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
    ));
    for (position, (expression, target)) in [(landed_float, f64_type), (landed_float, i8_type)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(
                &program,
                expression,
                target,
                position + 6,
            ),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(
                position + 6
            )),
        );
    }
    let zero = program
        .expression_table
        .insert(ExpressionNode::ZeroValue(bool_type));
    let string = program
        .expression_table
        .insert(ExpressionNode::String(Arc::from(&b"value"[..])));
    let mutable_byte_view =
        byte_slice_reference_type(&mut program, language_core::ReferenceAccess::Mutable);
    let undersized_buffer = bounded_byte_buffer_type(&mut program, 4);
    let bare_byte_array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: u8_type,
            length: FixedArrayLength::Literal(5),
        });
    for (position, target) in [
        (8, bool_type),
        (9, mutable_byte_view),
        (10, undersized_buffer),
        (11, bare_byte_array),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, string, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let array_values = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let array = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(array_values));
    assert_eq!(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, array, bool_type, 12),
        Err(RelationPlanError::DirectLiftLiteralTargetMismatch(12)),
    );
    let computed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: cases[4].0,
            operator: BinaryOperator::Equal,
            right: cases[4].0,
        }));
    let call_arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let call = program
        .expression_table
        .insert(ExpressionNode::Call(call_with_arguments(call_arguments)));
    for (position, (expression, target)) in
        [(zero, bool_type), (computed, bool_type), (call, bool_type)]
            .into_iter()
            .enumerate()
    {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(
                &program,
                expression,
                target,
                position + 13,
            ),
            Ok(None),
        );
    }

    let constrained = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: bool_type,
            constraints: HandleSpan::empty(),
        });
    let generic = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: SymbolHandle::invalid(),
            base_name: Identifier::generated_static("Box"),
            lifetime_arguments: Vec::new(),
            arguments: HandleSpan::empty(),
        });
    for (position, target) in [(16, constrained), (17, generic)] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, cases[4].0, target, position,),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let short_array = canonical_byte_array_literal(&mut program, b"four");
    let mismatched_landing_element = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(1).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::U16,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let mismatched_landing_elements = program
        .expression_table
        .insert_expression_handles([mismatched_landing_element]);
    let mismatched_landing_array = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(mismatched_landing_elements));
    let one_byte_array = fixed_byte_array_type(&mut program, 1);
    for (position, expression, target) in [
        (18, short_array, bare_byte_array),
        (19, mismatched_landing_array, one_byte_array),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let integer_array = canonical_byte_array_literal(&mut program, &[1]);
    let boolean_array = boolean_array_literal(&mut program, &[true]);
    let one_boolean_array = fixed_boolean_array_type(&mut program, 1);
    let two_boolean_array = fixed_boolean_array_type(&mut program, 2);
    for (position, expression, target) in [
        (20, integer_array, one_boolean_array),
        (21, boolean_array, one_byte_array),
        (22, boolean_array, two_boolean_array),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let out_of_range_array = integer_array_literal(&mut program, [IntegerLiteral::from_value(128)]);
    let wrapping_array = integer_array_literal(
        &mut program,
        [IntegerLiteral::from_value(1).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I8,
            domain: ArithmeticDomain::Wrapping,
        })],
    );
    let one_i8_array = fixed_integer_array_type(&mut program, "i8", 1);
    let constrained_boolean_array =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_i8_array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: i8_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(980),
                name: Identifier::generated_static("N"),
            },
        });
    for (position, expression, target) in [
        (23, out_of_range_array, one_i8_array),
        (24, wrapping_array, one_i8_array),
        (25, boolean_array, constrained_boolean_array),
        (26, out_of_range_array, unresolved_i8_array),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let mismatched_float_array = float_array_literal(
        &mut program,
        [FloatLiteral::parse("1.25f64").expect("format-landed f64 literal")],
    );
    let short_float_array = float_array_literal(
        &mut program,
        [FloatLiteral::parse("1.25f32").expect("format-landed f32 literal")],
    );
    let computed_left = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.0f32").expect("format-landed f32 literal"),
    ));
    let computed_right = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("2.0f32").expect("format-landed f32 literal"),
    ));
    let computed_float =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: computed_left,
                operator: BinaryOperator::Add,
                right: computed_right,
            }));
    let computed_elements = program
        .expression_table
        .insert_expression_handles([computed_float]);
    let computed_float_array = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(computed_elements));
    let one_f32_array = fixed_float_array_type(&mut program, "f32", 1);
    let two_f32_array = fixed_float_array_type(&mut program, "f32", 2);
    let constrained_f32 = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: f32_type,
            constraints: HandleSpan::empty(),
        });
    let constrained_float_array =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_f32,
                length: FixedArrayLength::Literal(1),
            });
    for (position, expression, target) in [
        (27, mismatched_float_array, one_f32_array),
        (28, integer_array, one_f32_array),
        (29, short_float_array, two_f32_array),
        (30, computed_float_array, one_f32_array),
        (31, short_float_array, constrained_float_array),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let exact_nested_boolean_array =
        nested_boolean_array_literal(&mut program, &[&[true], &[false]]);
    let ragged_nested_boolean_array =
        nested_boolean_array_literal(&mut program, &[&[true], &[false, true]]);
    let exact_nested_boolean_type = fixed_nested_boolean_array_type(&mut program, 2, 1);
    let wrong_outer_width_nested_boolean_type = fixed_nested_boolean_array_type(&mut program, 1, 1);
    let constrained_boolean_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_nested_boolean_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_boolean_row,
                length: FixedArrayLength::Literal(2),
            });
    let unresolved_boolean_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: bool_type,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(984),
                    name: Identifier::generated_static("M"),
                },
            });
    let unresolved_inner_width_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_boolean_row,
                length: FixedArrayLength::Literal(2),
            });
    let exact_boolean_row = fixed_boolean_array_type(&mut program, 1);
    let unresolved_outer_width_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_boolean_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(985),
                    name: Identifier::generated_static("N"),
                },
            });
    let nested_integer_array = {
        let row = integer_array_literal(&mut program, [IntegerLiteral::from_value(1)]);
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let computed_nested_boolean_array = {
        let row = program
            .expression_table
            .insert_expression_handles([computed]);
        let row = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(row));
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let one_by_one_nested_boolean_type = fixed_nested_boolean_array_type(&mut program, 1, 1);
    for (position, expression, target) in [
        (
            32,
            exact_nested_boolean_array,
            wrong_outer_width_nested_boolean_type,
        ),
        (33, ragged_nested_boolean_array, exact_nested_boolean_type),
        (
            34,
            exact_nested_boolean_array,
            constrained_nested_boolean_type,
        ),
        (35, exact_nested_boolean_array, unresolved_inner_width_type),
        (36, exact_nested_boolean_array, unresolved_outer_width_type),
        (
            38,
            computed_nested_boolean_array,
            one_by_one_nested_boolean_type,
        ),
        (40, nested_integer_array, one_by_one_nested_boolean_type),
        (41, boolean_array, one_by_one_nested_boolean_type),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let exact_nested_byte_array = nested_canonical_byte_array_literal(&mut program, &[&[1], &[2]]);
    let ragged_nested_byte_array =
        nested_canonical_byte_array_literal(&mut program, &[&[1], &[2, 3]]);
    let exact_nested_byte_type = fixed_nested_byte_array_type(&mut program, 2, 1);
    let wrong_outer_width_nested_byte_type = fixed_nested_byte_array_type(&mut program, 1, 1);
    let landed_byte_array = {
        let row = integer_array_literal(
            &mut program,
            [IntegerLiteral::from_value(1).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U8,
                domain: ArithmeticDomain::Exact,
            })],
        );
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let out_of_range_nested_byte_array = {
        let row = integer_array_literal(&mut program, [IntegerLiteral::from_value(256)]);
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let computed_nested_byte_array = {
        let left = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let right = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let computed =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Add,
                    right,
                }));
        let row = program
            .expression_table
            .insert_expression_handles([computed]);
        let row = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(row));
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let nested_boolean_array = nested_boolean_array_literal(&mut program, &[&[true]]);
    let constrained_u8 = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: u8_type,
            constraints: HandleSpan::empty(),
        });
    let constrained_byte_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: constrained_u8,
            length: FixedArrayLength::Literal(1),
        });
    let constrained_nested_byte_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_byte_row,
                length: FixedArrayLength::Literal(2),
            });
    let unresolved_byte_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: u8_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(988),
                name: Identifier::generated_static("M"),
            },
        });
    let unresolved_inner_byte_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_byte_row,
                length: FixedArrayLength::Literal(2),
            });
    let exact_byte_row = fixed_byte_array_type(&mut program, 1);
    let unresolved_outer_byte_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_byte_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(989),
                    name: Identifier::generated_static("N"),
                },
            });
    let one_by_one_nested_byte_type = fixed_nested_byte_array_type(&mut program, 1, 1);
    for (position, expression, target) in [
        (
            42,
            exact_nested_byte_array,
            wrong_outer_width_nested_byte_type,
        ),
        (43, ragged_nested_byte_array, exact_nested_byte_type),
        (44, landed_byte_array, one_by_one_nested_byte_type),
        (
            45,
            out_of_range_nested_byte_array,
            one_by_one_nested_byte_type,
        ),
        (46, computed_nested_byte_array, one_by_one_nested_byte_type),
        (47, nested_boolean_array, one_by_one_nested_byte_type),
        (48, exact_nested_byte_array, constrained_nested_byte_type),
        (49, exact_nested_byte_array, unresolved_inner_byte_type),
        (50, exact_nested_byte_array, unresolved_outer_byte_type),
        (52, integer_array, one_by_one_nested_byte_type),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let one_by_one_nested_i8_type = fixed_nested_integer_array_type(&mut program, "i8", 1, 1);
    let two_by_one_nested_i8_type = fixed_nested_integer_array_type(&mut program, "i8", 2, 1);
    let out_of_range_nested_integer_array =
        nested_integer_array_literal(&mut program, vec![vec![IntegerLiteral::from_value(128)]]);
    let mismatched_nested_integer_array = nested_integer_array_literal(
        &mut program,
        vec![vec![IntegerLiteral::from_value(1).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::U16,
                domain: ArithmeticDomain::Exact,
            },
        )]],
    );
    let wrapping_nested_integer_array = nested_integer_array_literal(
        &mut program,
        vec![vec![IntegerLiteral::from_value(1).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::I8,
                domain: ArithmeticDomain::Wrapping,
            },
        )]],
    );
    let ragged_nested_integer_array = nested_integer_array_literal(
        &mut program,
        vec![
            vec![IntegerLiteral::from_value(1)],
            vec![IntegerLiteral::from_value(2), IntegerLiteral::from_value(3)],
        ],
    );
    let nested_float_array = {
        let row = float_array_literal(
            &mut program,
            [FloatLiteral::parse("1.0f32").expect("format-landed f32 literal")],
        );
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let constrained_i8 = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: i8_type,
            constraints: HandleSpan::empty(),
        });
    let constrained_i8_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: constrained_i8,
            length: FixedArrayLength::Literal(1),
        });
    let constrained_nested_i8_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_i8_row,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_i8_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: i8_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(992),
                name: Identifier::generated_static("M"),
            },
        });
    let unresolved_inner_i8_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_i8_row,
                length: FixedArrayLength::Literal(1),
            });
    let exact_i8_row = fixed_integer_array_type(&mut program, "i8", 1);
    let unresolved_outer_i8_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_i8_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(993),
                    name: Identifier::generated_static("N"),
                },
            });
    for (position, expression, target) in [
        (
            53,
            out_of_range_nested_integer_array,
            one_by_one_nested_i8_type,
        ),
        (
            54,
            mismatched_nested_integer_array,
            one_by_one_nested_i8_type,
        ),
        (55, wrapping_nested_integer_array, one_by_one_nested_i8_type),
        (56, ragged_nested_integer_array, two_by_one_nested_i8_type),
        (57, computed_nested_byte_array, one_by_one_nested_i8_type),
        (58, nested_float_array, one_by_one_nested_i8_type),
        (59, nested_boolean_array, one_by_one_nested_i8_type),
        (60, nested_integer_array, constrained_nested_i8_type),
        (61, nested_integer_array, unresolved_inner_i8_type),
        (62, nested_integer_array, unresolved_outer_i8_type),
        (64, integer_array, one_by_one_nested_i8_type),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let one_by_one_nested_f32_type = fixed_nested_float_array_type(&mut program, "f32", 1, 1);
    let two_by_one_nested_f32_type = fixed_nested_float_array_type(&mut program, "f32", 2, 1);
    let exact_nested_float_array = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
        ]],
    );
    let mismatched_nested_float_array = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
        ]],
    );
    let ragged_nested_float_array = nested_float_array_literal(
        &mut program,
        vec![
            vec![FloatLiteral::parse("1.0").expect("anonymous float literal")],
            vec![
                FloatLiteral::parse("2.0").expect("anonymous float literal"),
                FloatLiteral::parse("3.0").expect("anonymous float literal"),
            ],
        ],
    );
    let computed_nested_float_array = {
        let rows = program
            .expression_table
            .insert_expression_handles([computed_float_array]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let constrained_float_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_f32,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_nested_float_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_float_row,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_float_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: f32_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(996),
                name: Identifier::generated_static("M"),
            },
        });
    let unresolved_inner_float_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_float_row,
                length: FixedArrayLength::Literal(1),
            });
    let exact_float_row = fixed_float_array_type(&mut program, "f32", 1);
    let unresolved_outer_float_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_float_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(997),
                    name: Identifier::generated_static("N"),
                },
            });
    let wrong_outer_width_nested_f32_type =
        fixed_nested_float_array_type(&mut program, "f32", 2, 1);
    for (position, expression, target) in [
        (
            65,
            exact_nested_float_array,
            wrong_outer_width_nested_f32_type,
        ),
        (
            66,
            mismatched_nested_float_array,
            one_by_one_nested_f32_type,
        ),
        (67, ragged_nested_float_array, two_by_one_nested_f32_type),
        (68, computed_nested_float_array, one_by_one_nested_f32_type),
        (69, nested_integer_array, one_by_one_nested_f32_type),
        (70, nested_boolean_array, one_by_one_nested_f32_type),
        (71, exact_nested_float_array, constrained_nested_float_type),
        (72, exact_nested_float_array, unresolved_inner_float_type),
        (73, exact_nested_float_array, unresolved_outer_float_type),
        (75, short_float_array, one_by_one_nested_f32_type),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let exact_tensor = boolean_tensor3_literal(&mut program, vec![vec![vec![true]]]);
    let two_plane_tensor_type = fixed_boolean_tensor3_type(&mut program, 2, 1, 1);
    let tall_plane_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 2, 1);
    let wide_row_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 1, 2);
    let exact_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 1, 1);
    let ragged_planes_tensor = boolean_tensor3_literal(
        &mut program,
        vec![vec![vec![true]], vec![vec![false], vec![true]]],
    );
    let ragged_rows_tensor =
        boolean_tensor3_literal(&mut program, vec![vec![vec![true], vec![false, true]]]);
    let numeric_tensor = {
        let planes = program
            .expression_table
            .insert_expression_handles([nested_integer_array]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(planes))
    };
    let computed_tensor = {
        let row = program
            .expression_table
            .insert_expression_handles([computed]);
        let row = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(row));
        let plane = program.expression_table.insert_expression_handles([row]);
        let plane = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(plane));
        let planes = program.expression_table.insert_expression_handles([plane]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(planes))
    };
    let constrained_tensor_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_tensor_plane =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_tensor_row,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_tensor_plane,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_tensor_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: bool_type,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(1000),
                    name: Identifier::generated_static("K"),
                },
            });
    let unresolved_row_tensor_plane =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_tensor_row,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_row_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_row_tensor_plane,
                length: FixedArrayLength::Literal(1),
            });
    let exact_tensor_row = fixed_boolean_array_type(&mut program, 1);
    let unresolved_plane_height =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_tensor_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(1001),
                    name: Identifier::generated_static("M"),
                },
            });
    let unresolved_plane_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_plane_height,
                length: FixedArrayLength::Literal(1),
            });
    let exact_tensor_plane = fixed_nested_boolean_array_type(&mut program, 1, 1);
    let unresolved_outer_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_tensor_plane,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(1002),
                    name: Identifier::generated_static("N"),
                },
            });
    for (position, expression, target) in [
        (76, exact_tensor, two_plane_tensor_type),
        (77, exact_tensor, tall_plane_tensor_type),
        (78, exact_tensor, wide_row_tensor_type),
        (79, ragged_planes_tensor, two_plane_tensor_type),
        (80, ragged_rows_tensor, tall_plane_tensor_type),
        (81, nested_boolean_array, exact_tensor_type),
        (82, numeric_tensor, exact_tensor_type),
        (83, computed_tensor, exact_tensor_type),
        (84, exact_tensor, constrained_tensor_type),
        (85, exact_tensor, unresolved_row_tensor_type),
        (86, exact_tensor, unresolved_plane_tensor_type),
        (87, exact_tensor, unresolved_outer_tensor_type),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let byte_tensor_target = {
        let matrix = fixed_nested_byte_array_type(&mut program, 1, 1);
        wrap_fixed_array_type(&mut program, matrix, 1)
    };
    let noncanonical_byte_tensor = wrap_array_literal(&mut program, [landed_byte_array]);
    let integer_tensor_target = {
        let matrix = fixed_nested_integer_array_type(&mut program, "i8", 1, 1);
        wrap_fixed_array_type(&mut program, matrix, 1)
    };
    let out_of_range_integer_tensor =
        wrap_array_literal(&mut program, [out_of_range_nested_integer_array]);
    let float_tensor_target = {
        let matrix = fixed_nested_float_array_type(&mut program, "f32", 1, 1);
        wrap_fixed_array_type(&mut program, matrix, 1)
    };
    let mismatched_float_tensor = wrap_array_literal(&mut program, [mismatched_nested_float_array]);
    let ragged_depth_four_target = wrap_fixed_array_type(&mut program, two_plane_tensor_type, 1);
    let ragged_depth_four = wrap_array_literal(&mut program, [ragged_planes_tensor]);
    let computed_depth_four_target = wrap_fixed_array_type(&mut program, exact_tensor_type, 1);
    let computed_depth_four = wrap_array_literal(&mut program, [computed_tensor]);
    for (position, expression, target) in [
        (89, noncanonical_byte_tensor, byte_tensor_target),
        (90, out_of_range_integer_tensor, integer_tensor_target),
        (91, mismatched_float_tensor, float_tensor_target),
        (92, ragged_depth_four, ragged_depth_four_target),
        (93, computed_depth_four, computed_depth_four_target),
    ] {
        assert_eq!(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
}

#[test]
fn direct_lift_literal_rejects_mutable_or_attached_representative_destinations() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(922),
        "LiteralQ",
        symbol(923),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let bool_type = primitive_type(&mut program, "bool");
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let arguments = program
        .expression_table
        .insert_expression_handles([boolean]);
    let call = call_with_arguments(arguments);
    let relation = ExactQuotientRelation {
        quotient_type: quotient,
        quotient_symbol: symbol(922),
        relation_symbol: symbol(923),
    };
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    for (is_mutable, is_self) in [(true, false), (false, true)] {
        let representative = RepresentativeTelescope {
            machine_symbol: symbol(924),
            state_symbol: symbol(925),
            parameters: vec![RepresentativeRuntimeParameter {
                symbol: symbol(926),
                type_reference: bool_type,
                is_mutable,
                is_self,
            }],
            return_type: carrier,
            machine_contracts: HandleSpan::empty(),
            state_contracts: HandleSpan::empty(),
            static_application: RepresentativeStaticApplication {
                lifetime_arguments: Vec::new(),
                bindings: Vec::new(),
            },
        };
        assert_eq!(
            derive_direct_lift_runtime_correspondence(
                &program,
                &Machine::default(),
                &state,
                &call,
                &[InputRelation::ExactEquality(bool_type)],
                relation,
                &representative,
            ),
            Err(RelationPlanError::DirectLiftParameterModeMismatch(0)),
        );
    }
}

#[test]
fn define_does_not_admit_closed_literal_arguments() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(927),
        "LiteralQ",
        symbol(928),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let bool_type = primitive_type(&mut program, "bool");
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let arguments = program
        .expression_table
        .insert_expression_handles([boolean]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let mut request = push_representative(&mut program, &[(bool_type, false, false)], carrier);
    request.kind = QuotientOperationKind::Define;

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &program,
            &Machine::default(),
            &state,
            &call,
            &request,
        ),
        Err(RelationPlanError::UnresolvedArgumentType(0)),
    );
}

#[test]
fn direct_lift_runtime_duplication_can_exceed_public_arity_without_granting_mutable_execution() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(
        &mut program,
        symbol(897),
        "DuplicatedQ",
        symbol(898),
        "DuplicatedR",
    );
    let carrier = carrier_type(&mut program);
    let public_symbol = symbol(899);
    let value = named_argument(&mut program, "value", public_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([value, value]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            is_mutable: true,
            ..Default::default()
        },
    );
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(900),
        state_symbol: symbol(901),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(902),
                type_reference: carrier,
                is_mutable: true,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(903),
                type_reference: carrier,
                is_mutable: true,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type,
        quotient_symbol: symbol(897),
        relation_symbol: symbol(898),
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::Quotient(relation),
    ];

    // This judgment retains correspondence only. Ordinary multiplicity,
    // custody, and call admission must still reject an executable duplicate
    // mutable occurrence at their owning layers.
    let runtime = derive_direct_lift_runtime_correspondence(
        &program,
        &Machine::default(),
        &state,
        &call,
        &input_relations,
        relation,
        &representative,
    )
    .expect("two representative positions may consume one public parameter");
    assert_eq!(runtime.positions.len(), 2);
    assert!(runtime.positions.iter().all(|position| position.source
        == crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(public_symbol)));
    assert_eq!(
        derive_define_runtime_correspondence(
            &program,
            &Machine::default(),
            &state,
            &call,
            &input_relations,
            relation,
            &representative,
        ),
        Err(RelationPlanError::DefineRuntimeArityMismatch),
    );
}

#[test]
fn direct_lift_runtime_rung_allows_a_zero_argument_representative_to_omit_every_public_parameter() {
    let mut program = TypedTrees::default();
    let result_quotient =
        quotient_type(&mut program, symbol(891), "ResultQ", symbol(892), "ResultR");
    let carrier = carrier_type(&mut program);
    let mut state = State {
        return_type: result_quotient,
        ..Default::default()
    };
    for (parameter, name, type_reference) in [
        (symbol(893), "unused_quotient", result_quotient),
        (symbol(894), "unused_ordinary", carrier),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(895),
        state_symbol: symbol(896),
        parameters: Vec::new(),
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let result_relation = ExactQuotientRelation {
        quotient_type: result_quotient,
        quotient_symbol: symbol(891),
        relation_symbol: symbol(892),
    };
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());

    let runtime = derive_direct_lift_runtime_correspondence(
        &program,
        &Machine::default(),
        &state,
        &call_with_arguments(arguments),
        &[],
        result_relation,
        &representative,
    )
    .expect("zero-argument lift may omit the complete public telescope");
    assert!(runtime.positions.is_empty());
}
