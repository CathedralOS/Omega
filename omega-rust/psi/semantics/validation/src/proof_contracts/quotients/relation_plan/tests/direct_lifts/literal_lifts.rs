use crate::proof_contracts::quotients::relation_plan::tests::{
    boolean_array_literal, boolean_tensor3_literal, bounded_byte_buffer_type,
    byte_slice_reference_type, call_with_arguments, canonical_byte_array_literal, carrier_type,
    exact_public_location, fixed_boolean_array_type, fixed_boolean_tensor3_type,
    fixed_byte_array_type, fixed_integer_array_type, fixed_nested_boolean_array_type,
    fixed_nested_byte_array_type, fixed_nested_float_array_type, fixed_nested_integer_array_type,
    integer_array_literal, named_argument, nested_boolean_array_literal,
    nested_canonical_byte_array_literal, nested_float_array_literal, nested_integer_array_literal,
    primitive_type, push_representative, quotient_type, symbol, wrap_array_literal,
    wrap_fixed_array_type,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema::{
    TheoremContractFactLocation, TheoremContractOwner, derive_expected_theorem_schema,
};
use crate::proof_contracts::quotients::relation_plan::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeRuntimeParameter,
    RepresentativeStaticApplication, RepresentativeTelescope, derive_define_runtime_correspondence,
    derive_direct_lift_precondition_implication, derive_direct_lift_public_precondition_partition,
    derive_direct_lift_runtime_correspondence, derive_direct_terminal_plan,
    derive_representative_precondition_partition,
};
use arena::HandleSpan;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{
    FloatFormat, FloatLiteral, IntegerLanding, IntegerLiteral, LandedIntegerType,
};
use std::sync::Arc;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;

#[test]
fn direct_lift_duplication_shares_each_side_value_without_collapsing_legality_coordinates() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(
        &mut program,
        symbol(904),
        "DiagonalQ",
        symbol(905),
        "DiagonalR",
    );
    let carrier = carrier_type(&mut program);
    let public_symbol = symbol(906);
    let unused_public_symbol = symbol(907);
    let public_value = named_argument(&mut program, "value", public_symbol);
    let public_diagonal = {
        let left = named_argument(&mut program, "value", public_symbol);
        let right = named_argument(&mut program, "value", public_symbol);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            }))
    };
    let public_facts = program.proof_facts.insert_many([
        ProofFact::Expression(public_diagonal),
        ProofFact::Expression(public_value),
    ]);
    let mut public_machine = Machine::default();
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: public_facts,
            ..Default::default()
        },
    );
    let mut public_state = State::default();
    for (symbol, name) in [(public_symbol, "value"), (unused_public_symbol, "unused")] {
        program.push_state_parameter(
            &mut public_state,
            StateParameter {
                symbol,
                name: Identifier::generated_static(name),
                type_reference: quotient_type,
                ..Default::default()
            },
        );
    }

    let representative_left = symbol(908);
    let representative_right = symbol(909);
    let representative_diagonal = {
        let left = named_argument(&mut program, "left", representative_left);
        let right = named_argument(&mut program, "right", representative_right);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            }))
    };
    let representative_left_value = named_argument(&mut program, "left", representative_left);
    let representative_right_value = named_argument(&mut program, "right", representative_right);
    let representative_facts = program.proof_facts.insert_many([
        ProofFact::Expression(representative_diagonal),
        ProofFact::Expression(representative_left_value),
        ProofFact::Expression(representative_right_value),
    ]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(910),
        state_symbol: symbol(911),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_left,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_right,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type,
        quotient_symbol: symbol(904),
        relation_symbol: symbol(905),
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::Quotient(relation),
    ];
    let runtime = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimeCorrespondence {
        positions: vec![
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(public_symbol),
                representative_parameter: representative_left,
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(public_symbol),
                representative_parameter: representative_right,
            },
        ],
    };
    let public_partition = derive_direct_lift_public_precondition_partition(
        &program,
        &public_machine,
        &public_state,
        &input_relations,
        &runtime,
    )
    .expect("duplicated and omitted quotient parameters remain dependent Q");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("both representative occurrences remain dependent P");
    let expected_theorem =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("duplication does not alter the universal positional theorem");
    assert_eq!(expected_theorem.parameters.len(), 4);
    assert_eq!(expected_theorem.relation_premises.len(), 2);
    assert_eq!(expected_theorem.left_application.arguments, [0, 2]);
    assert_eq!(expected_theorem.right_application.arguments, [1, 3]);

    let verified_theorem = crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremSchema {
        theorem_machine_symbol: symbol(912),
        theorem_state_symbol: symbol(913),
        parameters: expected_theorem
            .parameters
            .iter()
            .enumerate()
            .map(|(expected_position, _)| {
                crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremParameter {
                    expected_position,
                    theorem_symbol: symbol(920 + expected_position as u32),
                }
            })
            .collect(),
        relation_premises: expected_theorem
            .relation_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 20 + expected_position,
                    },
                },
            )
            .collect(),
        legality_premises: expected_theorem
            .legality_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 30 + expected_position,
                    },
                },
            )
            .collect(),
        conclusion: TheoremContractFactLocation {
            owner: TheoremContractOwner::State,
            contract_position: 0,
            fact_position: 0,
        },
    };

    let implication = derive_direct_lift_precondition_implication(
        &program,
        &public_machine,
        &public_state,
        &representative,
        &public_partition,
        &representative_partition,
        &runtime,
        &expected_theorem,
        &verified_theorem,
    )
    .expect("Q(x, x) and Q(x) instantiate both representative occurrences per side");
    assert_eq!(implication.rows.len(), 6);
    assert_eq!(
        exact_public_location(&implication.rows[1].proof).fact_position,
        1
    );
    assert_eq!(
        exact_public_location(&implication.rows[2].proof).fact_position,
        1
    );
    assert_eq!(implication.rows[1].representative.fact_position, 1);
    assert_eq!(implication.rows[2].representative.fact_position, 2);
    assert_ne!(implication.rows[1].theorem, implication.rows[2].theorem);
    assert_eq!(
        exact_public_location(&implication.rows[4].proof).fact_position,
        1
    );
    assert_eq!(
        exact_public_location(&implication.rows[5].proof).fact_position,
        1
    );
    assert_ne!(implication.rows[4].theorem, implication.rows[5].theorem);

    let mut tampered_runtime = runtime.clone();
    tampered_runtime.positions[1].source =
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(unused_public_symbol);
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &program,
            &public_machine,
            &public_state,
            &representative,
            &public_partition,
            &representative_partition,
            &tampered_runtime,
            &expected_theorem,
            &verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn direct_lift_runtime_rung_accepts_subsets_permutations_and_duplicates_but_rejects_adaptation() {
    let mut program = TypedTrees::default();
    let left_quotient = quotient_type(&mut program, symbol(880), "LeftQ", symbol(881), "LeftR");
    let right_quotient = quotient_type(&mut program, symbol(888), "RightQ", symbol(889), "RightR");
    let carrier = carrier_type(&mut program);
    let left_symbol = symbol(882);
    let right_symbol = symbol(883);
    let omitted_symbol = symbol(890);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let adapted = program
        .expression_table
        .insert(ExpressionNode::Integer(Default::default()));
    let mut state = State {
        return_type: left_quotient,
        ..Default::default()
    };
    for (symbol, name, type_reference) in [
        (left_symbol, "left", left_quotient),
        (right_symbol, "right", right_quotient),
        (omitted_symbol, "omitted", left_quotient),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(884),
        state_symbol: symbol(885),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(886),
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(887),
                type_reference: carrier,
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
    let left_relation = ExactQuotientRelation {
        quotient_type: left_quotient,
        quotient_symbol: symbol(880),
        relation_symbol: symbol(881),
    };
    let right_relation = ExactQuotientRelation {
        quotient_type: right_quotient,
        quotient_symbol: symbol(888),
        relation_symbol: symbol(889),
    };
    let derive = |program: &mut TypedTrees,
                  arguments: [ExpressionHandle; 2],
                  input_relations: [InputRelation; 2]| {
        let arguments = program
            .expression_table
            .insert_expression_handles(arguments);
        derive_direct_lift_runtime_correspondence(
            program,
            &Machine::default(),
            &state,
            &call_with_arguments(arguments),
            &input_relations,
            left_relation,
            &representative,
        )
    };

    assert!(
        derive(
            &mut program,
            [left, right],
            [
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
        )
        .is_ok(),
        "lift may select a direct subset of a larger public telescope"
    );
    let reordered = derive(
        &mut program,
        [right, left],
        [
            InputRelation::Quotient(right_relation),
            InputRelation::Quotient(left_relation),
        ],
    )
    .expect("lift may explicitly permute unique direct public parameters");
    assert_eq!(
        reordered.positions,
        [
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(right_symbol),
                representative_parameter: representative.parameters[0].symbol,
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(left_symbol),
                representative_parameter: representative.parameters[1].symbol,
            },
        ]
    );
    let duplicated = derive(
        &mut program,
        [left, left],
        [
            InputRelation::Quotient(left_relation),
            InputRelation::Quotient(left_relation),
        ],
    )
    .expect("lift may repeat one exact direct public parameter");
    assert_eq!(
        duplicated.positions,
        [
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(left_symbol),
                representative_parameter: representative.parameters[0].symbol,
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(left_symbol),
                representative_parameter: representative.parameters[1].symbol,
            },
        ]
    );
    assert_eq!(
        derive(
            &mut program,
            [left, adapted],
            [
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
        ),
        Err(RelationPlanError::DirectLiftLiteralTargetMismatch(1)),
    );
    assert_eq!(
        derive(
            &mut program,
            [right, left],
            [
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
        ),
        Err(RelationPlanError::DirectLiftParameterTypeMismatch(0)),
        "a permutation cannot retain the stale declaration-order relation vector"
    );

    let arguments = program
        .expression_table
        .insert_expression_handles([left, right]);
    assert_eq!(
        derive_define_runtime_correspondence(
            &program,
            &Machine::default(),
            &state,
            &call_with_arguments(arguments),
            &[
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
            left_relation,
            &representative,
        ),
        Err(RelationPlanError::DefineRuntimeArityMismatch),
        "define may not omit a public parameter"
    );
}

#[test]
fn direct_lift_runtime_accepts_only_closed_exact_scalar_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(920),
        "LiteralQ",
        symbol(921),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let bool_type = primitive_type(&mut program, "bool");
    let i8_type = primitive_type(&mut program, "i8");
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let integer = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
    let arguments = program
        .expression_table
        .insert_expression_handles([boolean, integer]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(bool_type, false, false), (i8_type, false, false)],
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
    .expect("the exact representative type lands an anonymous integer once");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(bool_type),
            InputRelation::ExactEquality(i8_type),
        ]
    );
    assert_eq!(plan.expected_theorem_schema.parameters.len(), 2);
    assert_eq!(
        plan.expected_theorem_schema.left_application.arguments,
        [0, 1]
    );
    assert_eq!(
        plan.expected_theorem_schema.right_application.arguments,
        [0, 1]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Boolean(true),
                ),
                representative_parameter: symbol(100),
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Integer {
                        spelling: "7".to_owned(),
                        landing: IntegerLanding {
                            landed_type: LandedIntegerType::I8,
                            domain: ArithmeticDomain::Exact,
                        },
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    let mut spelling_drift = runtime.clone();
    spelling_drift.positions[1].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Integer {
            spelling: "0x7".to_owned(),
            landing: IntegerLanding {
                landed_type: LandedIntegerType::I8,
                domain: ArithmeticDomain::Exact,
            },
        },
    );
    assert_ne!(
        runtime, spelling_drift,
        "literal spelling is retained identity"
    );
    let mut landing_drift = runtime.clone();
    landing_drift.positions[1].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Integer {
            spelling: "7".to_owned(),
            landing: IntegerLanding {
                landed_type: LandedIntegerType::I8,
                domain: ArithmeticDomain::Wrapping,
            },
        },
    );
    assert_ne!(
        runtime, landing_drift,
        "literal landing is retained identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_explicit_and_target_landed_float_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(970),
        "FloatLiteralQ",
        symbol(971),
        "FloatLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let f32_type = primitive_type(&mut program, "f32");
    let f64_type = primitive_type(&mut program, "f64");
    let f32_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
    ));
    let f64_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
    ));
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
        &[(f32_type, false, false), (f64_type, false, false)],
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
    .expect("the exact f32 target lands an anonymous decimal once");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(f32_type),
            InputRelation::ExactEquality(f64_type),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("float literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Float {
                        spelling: "1.25".to_owned(),
                        landing: FloatFormat::F32,
                    },
                ),
                representative_parameter: symbol(100),
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Float {
                        spelling: "1.25".to_owned(),
                        landing: FloatFormat::F64,
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    assert_ne!(
        runtime.positions[0].source, runtime.positions[1].source,
        "float format landing remains runtime-evidence identity"
    );
    let mut spelling_drift = runtime.clone();
    spelling_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Float {
            spelling: "1.5".to_owned(),
            landing: FloatFormat::F32,
        },
    );
    assert_ne!(
        runtime, spelling_drift,
        "float spelling remains runtime-evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_shared_and_bounded_byte_string_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(972),
        "ByteStringLiteralQ",
        symbol(973),
        "ByteStringLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let byte_view = byte_slice_reference_type(&mut program, language_core::ReferenceAccess::Shared);
    let bounded_bytes = bounded_byte_buffer_type(&mut program, 8);
    let literal_bytes: Arc<[u8]> = Arc::from(&b"value"[..]);
    let literal = program
        .expression_table
        .insert(ExpressionNode::String(literal_bytes.clone()));
    let arguments = program
        .expression_table
        .insert_expression_handles([literal, literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(byte_view, false, false), (bounded_bytes, false, false)],
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
    .expect("an immutable-image byte string has the exact shared byte-view type");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(byte_view),
            InputRelation::ExactEquality(bounded_bytes),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("byte-string literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::ByteString {
                        bytes: literal_bytes.clone(),
                        target_type: program.normalized_type_identity(byte_view),
                    },
                ),
                representative_parameter: symbol(100),
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::ByteString {
                        bytes: literal_bytes,
                        target_type: program.normalized_type_identity(bounded_bytes),
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    assert_ne!(
        runtime.positions[0].source, runtime.positions[1].source,
        "shared and owned bounded byte targets remain distinct identity"
    );
    let mut bytes_drift = runtime.clone();
    bytes_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::ByteString {
            bytes: Arc::from(&b"other"[..]),
            target_type: program.normalized_type_identity(byte_view),
        },
    );
    assert_ne!(
        runtime, bytes_drift,
        "exact bytes remain occurrence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_fixed_byte_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(974),
        "FixedByteArrayLiteralQ",
        symbol(975),
        "FixedByteArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_bytes = fixed_byte_array_type(&mut program, 5);
    let literal = canonical_byte_array_literal(&mut program, b"value");
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_bytes, false, false)], carrier);

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("ordinary contextual typing already landed the exact fixed byte array");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_bytes)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("fixed byte-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
            source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::FixedByteArray {
                    bytes: Arc::from(&b"value"[..]),
                    target_type: program.normalized_type_identity(fixed_bytes),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut bytes_drift = runtime.clone();
    bytes_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::FixedByteArray {
            bytes: Arc::from(&b"other"[..]),
            target_type: program.normalized_type_identity(fixed_bytes),
        },
    );
    assert_ne!(
        runtime, bytes_drift,
        "element order and values remain identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_nested_fixed_byte_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(986),
        "NestedFixedByteArrayLiteralQ",
        symbol(987),
        "NestedFixedByteArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_bytes = fixed_nested_byte_array_type(&mut program, 2, 3);
    let literal = nested_canonical_byte_array_literal(&mut program, &[&[0, 127, 255], &[3, 2, 1]]);
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_bytes, false, false)], carrier);

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("each exact byte row is already canonically context-landed");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_bytes)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested fixed-byte-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
            source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedFixedByteArray {
                    rows: Arc::from(vec![
                        Arc::from(&[0, 127, 255][..]),
                        Arc::from(&[3, 2, 1][..]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_bytes),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut row_boundary_drift = runtime.clone();
    row_boundary_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedFixedByteArray {
            rows: Arc::from(vec![
                Arc::from(&[0, 127][..]),
                Arc::from(&[255, 3, 2, 1][..]),
            ]),
            target_type: program.normalized_type_identity(fixed_bytes),
        },
    );
    assert_ne!(
        runtime, row_boundary_drift,
        "byte row boundaries remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_boolean_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(976),
        "BooleanArrayLiteralQ",
        symbol(977),
        "BooleanArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_booleans = fixed_boolean_array_type(&mut program, 3);
    let literal = boolean_array_literal(&mut program, &[true, false, true]);
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_booleans, false, false)], carrier);

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("an exact fixed Boolean array needs no element adaptation");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_booleans)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("Boolean-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
            source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::BooleanArray {
                    values: Arc::from(&[true, false, true][..]),
                    target_type: program.normalized_type_identity(fixed_booleans),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut order_drift = runtime.clone();
    order_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::BooleanArray {
            values: Arc::from(&[true, true, false][..]),
            target_type: program.normalized_type_identity(fixed_booleans),
        },
    );
    assert_ne!(
        runtime, order_drift,
        "Boolean element order remains identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_nested_boolean_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(982),
        "NestedBooleanArrayLiteralQ",
        symbol(983),
        "NestedBooleanArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_booleans = fixed_nested_boolean_array_type(&mut program, 2, 3);
    let literal =
        nested_boolean_array_literal(&mut program, &[&[true, false, true], &[false, true, false]]);
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_booleans, false, false)], carrier);

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("an exact depth-two Boolean array needs no adaptation");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_booleans)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested Boolean-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
            source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedBooleanArray {
                    rows: Arc::from(vec![
                        Arc::from(&[true, false, true][..]),
                        Arc::from(&[false, true, false][..]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_booleans),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut row_boundary_drift = runtime.clone();
    row_boundary_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedBooleanArray {
            rows: Arc::from(vec![
                Arc::from(&[true, false][..]),
                Arc::from(&[true, false, true, false][..]),
            ]),
            target_type: program.normalized_type_identity(fixed_booleans),
        },
    );
    assert_ne!(
        runtime, row_boundary_drift,
        "row boundaries remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_boolean_tensor3_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(998),
        "BooleanTensor3LiteralQ",
        symbol(999),
        "BooleanTensor3LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let tensor_type = fixed_boolean_tensor3_type(&mut program, 2, 2, 2);
    let literal = boolean_tensor3_literal(
        &mut program,
        vec![
            vec![vec![true, false], vec![false, true]],
            vec![vec![true, true], vec![false, false]],
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
    let request = push_representative(&mut program, &[(tensor_type, false, false)], carrier);

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("an exact depth-three Boolean tensor needs no adaptation");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(tensor_type)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("Boolean tensor runtime correspondence");
    assert_eq!(
        runtime.positions,
        [crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
            source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::BooleanTensor3 {
                    planes: Arc::from(vec![
                        Arc::from(vec![
                            Arc::from(&[true, false][..]),
                            Arc::from(&[false, true][..]),
                        ]),
                        Arc::from(vec![
                            Arc::from(&[true, true][..]),
                            Arc::from(&[false, false][..]),
                        ]),
                    ]),
                    target_type: program.normalized_type_identity(tensor_type),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut grouping_drift = runtime.clone();
    grouping_drift.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::BooleanTensor3 {
            planes: Arc::from(vec![
                Arc::from(vec![Arc::from(&[true, false][..])]),
                Arc::from(vec![
                    Arc::from(&[false, true][..]),
                    Arc::from(&[true, true][..]),
                    Arc::from(&[false, false][..]),
                ]),
            ]),
            target_type: program.normalized_type_identity(tensor_type),
        },
    );
    assert_ne!(
        runtime, grouping_drift,
        "plane and row boundaries remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_remaining_recursive_primitive_arrays() {
    use crate::proof_contracts::quotients::relation_plan::runtime_correspondence::{
        ClosedFloatArrayElement, ClosedIntegerArrayElement, ClosedRecursiveArrayElement as Value,
    };

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(1003),
        "RecursivePrimitiveArrayQ",
        symbol(1004),
        "RecursivePrimitiveArrayR",
    );
    let carrier = carrier_type(&mut program);

    let byte_matrix_type = fixed_nested_byte_array_type(&mut program, 1, 2);
    let byte_tensor_type = wrap_fixed_array_type(&mut program, byte_matrix_type, 2);
    let first_bytes = nested_canonical_byte_array_literal(&mut program, &[&[1, 2]]);
    let second_bytes = nested_canonical_byte_array_literal(&mut program, &[&[3, 4]]);
    let byte_tensor = wrap_array_literal(&mut program, [first_bytes, second_bytes]);

    let integer_matrix_type = fixed_nested_integer_array_type(&mut program, "i8", 1, 1);
    let integer_tensor_type = wrap_fixed_array_type(&mut program, integer_matrix_type, 1);
    let integer_matrix =
        nested_integer_array_literal(&mut program, vec![vec![IntegerLiteral::from_value(-1)]]);
    let integer_tensor = wrap_array_literal(&mut program, [integer_matrix]);

    let float_matrix_type = fixed_nested_float_array_type(&mut program, "f32", 1, 1);
    let float_tensor_type = wrap_fixed_array_type(&mut program, float_matrix_type, 1);
    let float_matrix = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
        ]],
    );
    let float_tensor = wrap_array_literal(&mut program, [float_matrix]);

    let boolean_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 1, 1);
    let boolean_depth_four_type = wrap_fixed_array_type(&mut program, boolean_tensor_type, 1);
    let boolean_tensor = boolean_tensor3_literal(&mut program, vec![vec![vec![true]]]);
    let boolean_depth_four = wrap_array_literal(&mut program, [boolean_tensor]);

    let arguments = program.expression_table.insert_expression_handles([
        byte_tensor,
        integer_tensor,
        float_tensor,
        boolean_depth_four,
    ]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[
            (byte_tensor_type, false, false),
            (integer_tensor_type, false, false),
            (float_tensor_type, false, false),
            (boolean_depth_four_type, false, false),
        ],
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
    .expect("remaining exact primitive arrays use recursive evidence");
    let runtime = plan
        .direct_lift_correspondence
        .expect("recursive primitive-array runtime correspondence");
    let i8_landing = IntegerLanding {
        landed_type: LandedIntegerType::I8,
        domain: ArithmeticDomain::Exact,
    };
    let expected = [
        (
            vec![
                Value::Array(Arc::from(vec![Value::Array(Arc::from(vec![
                    Value::Byte(1),
                    Value::Byte(2),
                ]))])),
                Value::Array(Arc::from(vec![Value::Array(Arc::from(vec![
                    Value::Byte(3),
                    Value::Byte(4),
                ]))])),
            ],
            program.normalized_type_identity(byte_tensor_type),
        ),
        (
            vec![Value::Array(Arc::from(vec![Value::Array(Arc::from(
                vec![Value::Integer(ClosedIntegerArrayElement {
                    spelling: "-1".to_owned(),
                    landing: i8_landing,
                })],
            ))]))],
            program.normalized_type_identity(integer_tensor_type),
        ),
        (
            vec![Value::Array(Arc::from(vec![Value::Array(Arc::from(
                vec![Value::Float(ClosedFloatArrayElement {
                    spelling: "1.25".to_owned(),
                    landing: FloatFormat::F32,
                })],
            ))]))],
            program.normalized_type_identity(float_tensor_type),
        ),
        (
            vec![Value::Array(Arc::from(vec![Value::Array(Arc::from(
                vec![Value::Array(Arc::from(vec![Value::Boolean(true)]))],
            ))]))],
            program.normalized_type_identity(boolean_depth_four_type),
        ),
    ];
    for (position, ((expected_elements, expected_type), actual)) in
        expected.into_iter().zip(&runtime.positions).enumerate()
    {
        let crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::RecursivePrimitiveArray {
                elements,
                target_type,
            },
        ) = &actual.source
        else {
            panic!("recursive fallback owns newly admitted position {position}");
        };
        assert_eq!(elements.as_ref(), expected_elements);
        assert_eq!(*target_type, expected_type);
    }

    assert!(matches!(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(
            &program,
            boolean_tensor,
            boolean_tensor_type,
            4,
        ),
        Ok(Some(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::BooleanTensor3 { .. }
        ))
    ));
    assert!(matches!(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(&program, float_matrix, float_matrix_type, 5,),
        Ok(Some(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::NestedFloatArray { .. }
        ))
    ));
    let boolean_depth_five_type = wrap_fixed_array_type(&mut program, boolean_depth_four_type, 1);
    let boolean_depth_five = wrap_array_literal(&mut program, [boolean_depth_four]);
    assert!(matches!(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::closed_lift_literal_for_representative(
            &program,
            boolean_depth_five,
            boolean_depth_five_type,
            6,
        ),
        Ok(Some(
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::RecursivePrimitiveArray { .. }
        ))
    ));
}

#[test]
fn direct_lift_runtime_accepts_exact_integer_array_literals() {
    use crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedIntegerArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(978),
        "IntegerArrayLiteralQ",
        symbol(979),
        "IntegerArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let i8_array = fixed_integer_array_type(&mut program, "i8", 2);
    let u16_array = fixed_integer_array_type(&mut program, "u16", 1);
    let i8_literal = integer_array_literal(
        &mut program,
        [
            IntegerLiteral::from_value(-1),
            IntegerLiteral::from_value(127),
        ],
    );
    let u16_landing = IntegerLanding {
        landed_type: LandedIntegerType::U16,
        domain: ArithmeticDomain::Exact,
    };
    let u16_literal = integer_array_literal(
        &mut program,
        [IntegerLiteral::from_value(7).with_landing(u16_landing)],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([i8_literal, u16_literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(i8_array, false, false), (u16_array, false, false)],
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
    .expect("each fixed integer-array element lands by the exact scalar rule");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(i8_array),
            InputRelation::ExactEquality(u16_array),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("integer-array literal runtime correspondence");
    let i8_landing = IntegerLanding {
        landed_type: LandedIntegerType::I8,
        domain: ArithmeticDomain::Exact,
    };
    assert_eq!(
        runtime.positions,
        [
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::IntegerArray {
                        elements: Arc::from(vec![
                            ClosedIntegerArrayElement {
                                spelling: "-1".to_owned(),
                                landing: i8_landing,
                            },
                            ClosedIntegerArrayElement {
                                spelling: "127".to_owned(),
                                landing: i8_landing,
                            },
                        ]),
                        target_type: program.normalized_type_identity(i8_array),
                    },
                ),
                representative_parameter: symbol(100),
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::IntegerArray {
                        elements: Arc::from(vec![ClosedIntegerArrayElement {
                            spelling: "7".to_owned(),
                            landing: u16_landing,
                        }]),
                        target_type: program.normalized_type_identity(u16_array),
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    assert_ne!(
        runtime.positions[0].source, runtime.positions[1].source,
        "element landing and exact array target remain identity"
    );
    let mut landing_drift = runtime.clone();
    landing_drift.positions[1].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::IntegerArray {
            elements: Arc::from(vec![ClosedIntegerArrayElement {
                spelling: "7".to_owned(),
                landing: IntegerLanding {
                    landed_type: LandedIntegerType::U16,
                    domain: ArithmeticDomain::Wrapping,
                },
            }]),
            target_type: program.normalized_type_identity(u16_array),
        },
    );
    assert_ne!(runtime, landing_drift, "element domain remains identity");
}
