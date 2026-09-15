use super::{
    arithmetic_implication_fixture, baseline_arithmetic_implication_fixture, binary_expression,
    boolean_array_literal, boolean_tensor3_literal, call_with_arguments,
    canonical_byte_array_literal, carrier_type, direct_lift_implication_fixture,
    exact_public_location, float_array_literal, integer_array_literal, named_argument,
    nested_boolean_array_literal, nested_canonical_byte_array_literal, nested_float_array_literal,
    nested_integer_array_literal, primitive_type, quotient_type, static_argument, symbol,
    wrap_array_literal,
};
use crate::proof_contracts::quotients::relation_plan::correspondence_certificate::{
    FixedRepresentativeCallPreconditions, QuotientCorrespondenceEvidence,
    compose_lift_correspondence_certificate,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema::{
    TheoremApplicationSide, TheoremContractFactLocation, TheoremContractOwner,
    derive_expected_theorem_schema,
};
use crate::proof_contracts::quotients::relation_plan::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeRuntimeParameter,
    RepresentativeStaticApplication, RepresentativeTelescope,
    derive_direct_lift_precondition_implication, derive_direct_lift_public_precondition_partition,
    derive_representative_precondition_partition,
};
use arena::HandleSpan;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{
    FloatFormat, FloatLiteral, IntegerLanding, IntegerLiteral, LandedIntegerType,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::{ProofFact, ProofMembershipFact};
use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::proposition::PropositionApplication;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;

#[test]
fn direct_lift_arithmetic_implication_rejects_proof_views_float_and_domain_drift() {
    let proof_view =
        arithmetic_implication_fixture(|program, public_symbol, representative_symbol, _, _| {
            let public = named_argument(program, "public", public_symbol);
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let q = binary_expression(program, public, BinaryOperator::Greater, zero);
            let argument = named_argument(program, "representative", representative_symbol);
            let arguments = program
                .expression_table
                .insert_expression_handles([argument]);
            let mut call = call_with_arguments(arguments);
            call.target = Identifier::generated_static("Bag");
            let view = program.expression_table.insert(ExpressionNode::Call(call));
            let p = binary_expression(program, view, BinaryOperator::Equal, view);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        });
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &proof_view.program,
            &proof_view.public_machine,
            &proof_view.public_state,
            &proof_view.representative,
            &proof_view.public_partition,
            &proof_view.representative_partition,
            &proof_view.runtime,
            &proof_view.expected_theorem,
            &proof_view.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let float =
        arithmetic_implication_fixture(|program, public_symbol, representative_symbol, _, _| {
            let public = named_argument(program, "public", public_symbol);
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let q = binary_expression(program, public, BinaryOperator::Greater, zero);
            let representative = named_argument(program, "representative", representative_symbol);
            // A decimal spelling without a landing is an anonymous rational,
            // not an IEEE value. This negative must retain a real float carrier.
            let value = program.expression_table.insert(ExpressionNode::Float(
                FloatLiteral::parse("0.0f64").unwrap(),
            ));
            let p = binary_expression(program, representative, BinaryOperator::Greater, value);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        });
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &float.program,
            &float.public_machine,
            &float.public_state,
            &float.representative,
            &float.public_partition,
            &float.representative_partition,
            &float.runtime,
            &float.expected_theorem,
            &float.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let mut domain_drift = baseline_arithmetic_implication_fixture();
    let crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Integer { landing, .. },
    ) = &mut domain_drift.runtime.positions[1].source
    else {
        panic!("integer literal fixture")
    };
    landing.domain = ArithmeticDomain::Wrapping;
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &domain_drift.program,
            &domain_drift.public_machine,
            &domain_drift.public_state,
            &domain_drift.representative,
            &domain_drift.public_partition,
            &domain_drift.representative_partition,
            &domain_drift.runtime,
            &domain_drift.expected_theorem,
            &domain_drift.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn direct_lift_q_implies_p_retains_both_exact_theorem_coordinates_and_allows_extra_q() {
    let fixture = direct_lift_implication_fixture(true);
    assert_eq!(fixture.public_partition.dependent.len(), 3);
    assert_eq!(fixture.representative_partition.dependent.len(), 1);
    let implication = derive_direct_lift_precondition_implication(
        &fixture.program,
        &fixture.public_machine,
        &fixture.public_state,
        &fixture.representative,
        &fixture.public_partition,
        &fixture.representative_partition,
        &fixture.runtime,
        &fixture.expected_theorem,
        &fixture.verified_theorem,
    )
    .expect("one matching Q fact may imply P while another Q fact remains extra");

    assert_eq!(implication.rows.len(), 2);
    assert_eq!(
        implication.rows[0].application,
        TheoremApplicationSide::Left
    );
    assert_eq!(
        implication.rows[1].application,
        TheoremApplicationSide::Right
    );
    assert_eq!(
        exact_public_location(&implication.rows[0].proof).fact_position,
        0
    );
    assert_eq!(implication.rows[0].representative.fact_position, 0);
    assert_eq!(implication.rows[0].theorem.fact_position, 1);
    assert_eq!(implication.rows[1].theorem.fact_position, 2);

    let certificate = compose_lift_correspondence_certificate(
        &Ok(fixture.verified_theorem.clone()),
        &fixture.runtime,
        &implication,
        &FixedRepresentativeCallPreconditions { rows: Vec::new() },
        &fixture.representative_partition,
    )
    .expect("verified theorem plus exact implication composes");
    let QuotientCorrespondenceEvidence::DirectLift {
        runtime,
        precondition,
        fixed,
    } = certificate.evidence
    else {
        panic!("direct lift evidence")
    };
    assert_eq!(runtime, fixture.runtime);
    assert_eq!(precondition, implication);
    assert!(fixed.rows.is_empty());
}

#[test]
fn direct_lift_q_implies_p_rejects_missing_identity_and_theorem_coordinate_tamper() {
    let missing = direct_lift_implication_fixture(false);
    assert_eq!(missing.public_partition.dependent.len(), 2);
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &missing.program,
            &missing.public_machine,
            &missing.public_state,
            &missing.representative,
            &missing.public_partition,
            &missing.representative_partition,
            &missing.runtime,
            &missing.expected_theorem,
            &missing.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let exact = direct_lift_implication_fixture(true);
    let mut tampered_theorem = exact.verified_theorem.clone();
    tampered_theorem.legality_premises.pop();
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &exact.program,
            &exact.public_machine,
            &exact.public_state,
            &exact.representative,
            &exact.public_partition,
            &exact.representative_partition,
            &exact.runtime,
            &exact.expected_theorem,
            &tampered_theorem,
        ),
        Err(RelationPlanError::DirectLiftTheoremLegalityMismatch),
    );
    assert!(
        compose_lift_correspondence_certificate(
            &Err(RelationPlanError::TheoremSchemaConclusionMismatch),
            &exact.runtime,
            &crate::proof_contracts::quotients::relation_plan::correspondence_certificate::DirectLiftPreconditionImplication { rows: Vec::new() },
            &FixedRepresentativeCallPreconditions { rows: Vec::new() },
            &exact.representative_partition,
        )
        .is_none()
    );
}

#[test]
fn direct_lift_literal_stays_fixed_and_dependent_use_requires_an_exact_public_fact() {
    let fixture = direct_lift_implication_fixture(true);
    let literal = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Boolean(true);

    let mut fixed_literal = fixture.runtime.clone();
    fixed_literal.positions[1].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(literal.clone());
    assert!(
        derive_direct_lift_precondition_implication(
            &fixture.program,
            &fixture.public_machine,
            &fixture.public_state,
            &fixture.representative,
            &fixture.public_partition,
            &fixture.representative_partition,
            &fixed_literal,
            &fixture.expected_theorem,
            &fixture.verified_theorem,
        )
        .is_ok(),
        "a literal-fed ordinary position remains a fixed call obligation"
    );

    let mut dependent_literal = fixture.runtime.clone();
    dependent_literal.positions[0].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(literal);
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &fixture.program,
            &fixture.public_machine,
            &fixture.public_state,
            &fixture.representative,
            &fixture.public_partition,
            &fixture.representative_partition,
            &dependent_literal,
            &fixture.expected_theorem,
            &fixture.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn direct_lift_literal_substitutes_exactly_inside_dependent_representative_p() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(930),
        "LiteralQ",
        symbol(931),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let i32_type = primitive_type(&mut program, "i32");
    let public_symbol = symbol(932);
    let representative_quotient = symbol(933);
    let representative_literal = symbol(934);
    let public_value = named_argument(&mut program, "public", public_symbol);
    let public_literal = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I32,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let public_fact =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: public_value,
                operator: BinaryOperator::Equal,
                right: public_literal,
            }));
    let public_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(public_fact)]);
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
    program.push_state_parameter(
        &mut public_state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("public"),
            type_reference: quotient,
            ..Default::default()
        },
    );

    let representative_value =
        named_argument(&mut program, "representative", representative_quotient);
    let representative_constant = named_argument(&mut program, "constant", representative_literal);
    let representative_fact =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: representative_value,
                operator: BinaryOperator::Equal,
                right: representative_constant,
            }));
    let representative_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(representative_fact)]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(935),
        state_symbol: symbol(936),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_quotient,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_literal,
                type_reference: i32_type,
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
        quotient_type: quotient,
        quotient_symbol: symbol(930),
        relation_symbol: symbol(931),
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::ExactEquality(i32_type),
    ];
    let runtime = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimeCorrespondence {
        positions: vec![
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(public_symbol),
                representative_parameter: representative_quotient,
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftRuntimePosition {
                source: crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
                    crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Integer {
                        spelling: "7".to_owned(),
                        landing: IntegerLanding {
                            landed_type: LandedIntegerType::I32,
                            domain: ArithmeticDomain::Exact,
                        },
                    },
                ),
                representative_parameter: representative_literal,
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
    .expect("mixed public Q partitions on its quotient value");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("mixed representative P partitions on its quotient value");
    assert_eq!(public_partition.dependent.len(), 1);
    assert_eq!(representative_partition.dependent.len(), 1);
    let expected =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("literal positions remain ordinary universal theorem positions");
    assert_eq!(expected.parameters.len(), 3);
    assert_eq!(expected.left_application.arguments, [0, 2]);
    assert_eq!(expected.right_application.arguments, [1, 2]);
    let verified = crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremSchema {
        theorem_machine_symbol: symbol(937),
        theorem_state_symbol: symbol(938),
        parameters: expected
            .parameters
            .iter()
            .enumerate()
            .map(|(expected_position, _)| {
                crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremParameter {
                    expected_position,
                    theorem_symbol: symbol(940 + u32::try_from(expected_position).unwrap()),
                }
            })
            .collect(),
        relation_premises: expected
            .relation_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 10 + expected_position,
                    },
                },
            )
            .collect(),
        legality_premises: expected
            .legality_premises
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
        &expected,
        &verified,
    )
    .expect("Q(public, 7i32) exactly contains P(representative, literal-fed parameter)");
    assert_eq!(implication.rows.len(), 2);
    assert_eq!(
        implication.rows[0].application,
        TheoremApplicationSide::Left
    );
    assert_eq!(
        implication.rows[1].application,
        TheoremApplicationSide::Right
    );
    assert_ne!(implication.rows[0].theorem, implication.rows[1].theorem);

    let mut drifted = runtime.clone();
    drifted.positions[1].source = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Integer {
            spelling: "8".to_owned(),
            landing: IntegerLanding {
                landed_type: LandedIntegerType::I32,
                domain: ArithmeticDomain::Exact,
            },
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &program,
            &public_machine,
            &public_state,
            &representative,
            &public_partition,
            &representative_partition,
            &drifted,
            &expected,
            &verified,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn proof_fact_literal_substitution_retains_value_landing_and_recursive_fact_shape() {
    use crate::proof_contracts::quotients::relation_plan::proof_fact_identity::{
        ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
    };

    let mut program = TypedTrees::default();
    let parameter = symbol(950);
    let parameter_value = named_argument(&mut program, "constant", parameter);
    let exact_landing = IntegerLanding {
        landed_type: LandedIntegerType::I32,
        domain: ArithmeticDomain::Exact,
    };
    let exact_integer = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(exact_landing),
    ));
    let wrapping_integer = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I32,
            domain: ArithmeticDomain::Wrapping,
        }),
    ));
    let no_values = Vec::new();
    let exact_value = vec![ProofValueSubstitution::integer(
        parameter,
        "7",
        exact_landing,
    )];
    let context = |values| ProofFactIdentityContext {
        values,
        static_bindings: &[],
    };
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(parameter_value),
        &ProofFact::Expression(exact_integer),
        context(&exact_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(exact_integer),
        &ProofFact::Expression(wrapping_integer),
        context(&no_values),
        context(&no_values),
    ));

    let proposition = symbol(951);
    let parameter_arguments = program
        .expression_table
        .insert_expression_handles([parameter_value]);
    let literal_arguments = program
        .expression_table
        .insert_expression_handles([exact_integer]);
    let proposition_fact = |arguments| {
        ProofFact::Proposition(PropositionApplication {
            proposition,
            name: Identifier::generated_static("ExactLiteral"),
            binder_arguments: Box::default(),
            arguments,
        })
    };
    assert!(proof_facts_match(
        &program,
        &proposition_fact(parameter_arguments),
        &proposition_fact(literal_arguments),
        context(&exact_value),
        context(&no_values),
    ));

    let domain_symbol = symbol(952);
    assert!(proof_facts_match(
        &program,
        &ProofFact::Membership(ProofMembershipFact {
            value: parameter_value,
            domain: HandleSpan::empty(),
            domain_symbol,
            authored_domain_selection: None,
            ..Default::default()
        }),
        &ProofFact::Membership(ProofMembershipFact {
            value: exact_integer,
            domain: HandleSpan::empty(),
            domain_symbol,
            authored_domain_selection: None,
            ..Default::default()
        }),
        context(&exact_value),
        context(&no_values),
    ));

    let boolean_parameter = symbol(953);
    let boolean_name = named_argument(&mut program, "flag", boolean_parameter);
    let true_literal = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let false_literal = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let true_value = vec![ProofValueSubstitution::boolean(boolean_parameter, true)];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_name),
        &ProofFact::Expression(true_literal),
        context(&true_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_name),
        &ProofFact::Expression(false_literal),
        context(&true_value),
        context(&no_values),
    ));

    let float_parameter = symbol(954);
    let float_name = named_argument(&mut program, "scale", float_parameter);
    let f32_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
    ));
    let f64_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
    ));
    let other_f32_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.5f32").expect("format-landed f32 literal"),
    ));
    let float_value = vec![ProofValueSubstitution::float(
        float_parameter,
        "1.25",
        FloatFormat::F32,
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(f32_literal),
        context(&float_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(f64_literal),
        context(&float_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(other_f32_literal),
        context(&float_value),
        context(&no_values),
    ));

    let string_parameter = symbol(955);
    let string_name = named_argument(&mut program, "label", string_parameter);
    let string_literal = program
        .expression_table
        .insert(ExpressionNode::String(Arc::from(&b"value"[..])));
    let other_string_literal = program
        .expression_table
        .insert(ExpressionNode::String(Arc::from(&b"other"[..])));
    let string_value = vec![ProofValueSubstitution::byte_string(
        string_parameter,
        b"value",
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(string_name),
        &ProofFact::Expression(string_literal),
        context(&string_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(string_name),
        &ProofFact::Expression(other_string_literal),
        context(&string_value),
        context(&no_values),
    ));

    let byte_array_parameter = symbol(956);
    let byte_array_name = named_argument(&mut program, "bytes", byte_array_parameter);
    let byte_array_literal = canonical_byte_array_literal(&mut program, b"value");
    let other_byte_array_literal = canonical_byte_array_literal(&mut program, b"other");
    let byte_array_value = vec![ProofValueSubstitution::fixed_byte_array(
        byte_array_parameter,
        b"value",
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(byte_array_name),
        &ProofFact::Expression(byte_array_literal),
        context(&byte_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(byte_array_name),
        &ProofFact::Expression(other_byte_array_literal),
        context(&byte_array_value),
        context(&no_values),
    ));

    let nested_byte_array_parameter = symbol(960);
    let nested_byte_array_name =
        named_argument(&mut program, "byte_rows", nested_byte_array_parameter);
    let exact_nested_byte_array_literal =
        nested_canonical_byte_array_literal(&mut program, &[&[1, 2], &[3, 4]]);
    let row_boundary_drifted_byte_array_literal =
        nested_canonical_byte_array_literal(&mut program, &[&[1], &[2, 3, 4]]);
    let flattened_byte_array_literal = canonical_byte_array_literal(&mut program, &[1, 2, 3, 4]);
    let nested_byte_array_value = vec![ProofValueSubstitution::nested_fixed_byte_array(
        nested_byte_array_parameter,
        &[Arc::from(&[1, 2][..]), Arc::from(&[3, 4][..])],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_byte_array_name),
        &ProofFact::Expression(exact_nested_byte_array_literal),
        context(&nested_byte_array_value),
        context(&no_values),
    ));
    assert!(
        !proof_facts_match(
            &program,
            &ProofFact::Expression(nested_byte_array_name),
            &ProofFact::Expression(row_boundary_drifted_byte_array_literal),
            context(&nested_byte_array_value),
            context(&no_values),
        ),
        "row-delimited traces retain byte-matrix boundaries"
    );
    assert!(
        !proof_facts_match(
            &program,
            &ProofFact::Expression(nested_byte_array_name),
            &ProofFact::Expression(flattened_byte_array_literal),
            context(&nested_byte_array_value),
            context(&no_values),
        ),
        "container-delimited traces distinguish nested and flat byte arrays"
    );

    let boolean_array_parameter = symbol(957);
    let boolean_array_name = named_argument(&mut program, "flags", boolean_array_parameter);
    let exact_boolean_array_literal = boolean_array_literal(&mut program, &[true, false]);
    let other_boolean_array_literal = boolean_array_literal(&mut program, &[false, true]);
    let boolean_array_value = vec![ProofValueSubstitution::boolean_array(
        boolean_array_parameter,
        &[true, false],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_array_name),
        &ProofFact::Expression(exact_boolean_array_literal),
        context(&boolean_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_array_name),
        &ProofFact::Expression(other_boolean_array_literal),
        context(&boolean_array_value),
        context(&no_values),
    ));

    let integer_array_parameter = symbol(958);
    let integer_array_name = named_argument(&mut program, "offsets", integer_array_parameter);
    let i16_landing = IntegerLanding {
        landed_type: LandedIntegerType::I16,
        domain: ArithmeticDomain::Exact,
    };
    let exact_integer_array_literal = integer_array_literal(
        &mut program,
        [
            IntegerLiteral::from_value(-1).with_landing(i16_landing),
            IntegerLiteral::from_value(7).with_landing(i16_landing),
        ],
    );
    let wrapping_integer_array_literal = integer_array_literal(
        &mut program,
        [
            IntegerLiteral::from_value(-1).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::I16,
                domain: ArithmeticDomain::Wrapping,
            }),
            IntegerLiteral::from_value(7).with_landing(i16_landing),
        ],
    );
    let integer_array_value = vec![ProofValueSubstitution::integer_array(
        integer_array_parameter,
        [
            ("-1".to_owned(), i16_landing),
            ("7".to_owned(), i16_landing),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(integer_array_name),
        &ProofFact::Expression(exact_integer_array_literal),
        context(&integer_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(integer_array_name),
        &ProofFact::Expression(wrapping_integer_array_literal),
        context(&integer_array_value),
        context(&no_values),
    ));

    let nested_integer_array_parameter = symbol(962);
    let nested_integer_array_name =
        named_argument(&mut program, "offset_rows", nested_integer_array_parameter);
    let landed_integer = |value| IntegerLiteral::from_value(value).with_landing(i16_landing);
    let exact_nested_integer_array_literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![landed_integer(-1), landed_integer(7)],
            vec![landed_integer(8), landed_integer(9)],
        ],
    );
    let row_boundary_drifted_integer_array_literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![landed_integer(-1)],
            vec![landed_integer(7), landed_integer(8), landed_integer(9)],
        ],
    );
    let flattened_integer_array_literal = integer_array_literal(
        &mut program,
        [
            landed_integer(-1),
            landed_integer(7),
            landed_integer(8),
            landed_integer(9),
        ],
    );
    let wrapping_nested_integer_array_literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![
                IntegerLiteral::from_value(-1).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::I16,
                    domain: ArithmeticDomain::Wrapping,
                }),
                landed_integer(7),
            ],
            vec![landed_integer(8), landed_integer(9)],
        ],
    );
    let nested_integer_array_value = vec![ProofValueSubstitution::nested_integer_array(
        nested_integer_array_parameter,
        &[
            Arc::from(vec![
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "-1".to_owned(),
                    landing: i16_landing,
                },
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "7".to_owned(),
                    landing: i16_landing,
                },
            ]),
            Arc::from(vec![
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "8".to_owned(),
                    landing: i16_landing,
                },
                crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "9".to_owned(),
                    landing: i16_landing,
                },
            ]),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_integer_array_name),
        &ProofFact::Expression(exact_nested_integer_array_literal),
        context(&nested_integer_array_value),
        context(&no_values),
    ));
    for drifted in [
        row_boundary_drifted_integer_array_literal,
        flattened_integer_array_literal,
        wrapping_nested_integer_array_literal,
    ] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(nested_integer_array_name),
            &ProofFact::Expression(drifted),
            context(&nested_integer_array_value),
            context(&no_values),
        ));
    }

    let float_array_parameter = symbol(959);
    let float_array_name = named_argument(&mut program, "scales", float_array_parameter);
    let exact_float_array_literal = float_array_literal(
        &mut program,
        [
            FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
            FloatLiteral::parse("2.5f32").expect("format-landed f32 literal"),
        ],
    );
    let drifted_float_array_literal = float_array_literal(
        &mut program,
        [
            FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
            FloatLiteral::parse("2.5f32").expect("format-landed f32 literal"),
        ],
    );
    let float_array_value = vec![ProofValueSubstitution::float_array(
        float_array_parameter,
        [
            ("1.25".to_owned(), FloatFormat::F32),
            ("2.5".to_owned(), FloatFormat::F32),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(float_array_name),
        &ProofFact::Expression(exact_float_array_literal),
        context(&float_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_array_name),
        &ProofFact::Expression(drifted_float_array_literal),
        context(&float_array_value),
        context(&no_values),
    ));

    let nested_float_array_parameter = symbol(963);
    let nested_float_array_name =
        named_argument(&mut program, "scale_rows", nested_float_array_parameter);
    let f32_literal = |text| FloatLiteral::parse(text).expect("format-landed f32 literal");
    let exact_nested_float_array_literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![f32_literal("1.25f32"), f32_literal("2.5f32")],
            vec![f32_literal("3.75f32"), f32_literal("4.5f32")],
        ],
    );
    let row_drifted_float_array_literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![f32_literal("1.25f32")],
            vec![
                f32_literal("2.5f32"),
                f32_literal("3.75f32"),
                f32_literal("4.5f32"),
            ],
        ],
    );
    let flattened_float_array_literal = float_array_literal(
        &mut program,
        [
            f32_literal("1.25f32"),
            f32_literal("2.5f32"),
            f32_literal("3.75f32"),
            f32_literal("4.5f32"),
        ],
    );
    let format_drifted_nested_float_array_literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![
                FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
                f32_literal("2.5f32"),
            ],
            vec![f32_literal("3.75f32"), f32_literal("4.5f32")],
        ],
    );
    let float_element = |spelling: &str| {
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedFloatArrayElement {
        spelling: spelling.to_owned(),
        landing: FloatFormat::F32,
    }
    };
    let nested_float_array_value = vec![ProofValueSubstitution::nested_float_array(
        nested_float_array_parameter,
        &[
            Arc::from(vec![float_element("1.25"), float_element("2.5")]),
            Arc::from(vec![float_element("3.75"), float_element("4.5")]),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_float_array_name),
        &ProofFact::Expression(exact_nested_float_array_literal),
        context(&nested_float_array_value),
        context(&no_values),
    ));
    for drifted in [
        row_drifted_float_array_literal,
        flattened_float_array_literal,
        format_drifted_nested_float_array_literal,
    ] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(nested_float_array_name),
            &ProofFact::Expression(drifted),
            context(&nested_float_array_value),
            context(&no_values),
        ));
    }

    let nested_boolean_array_parameter = symbol(960);
    let nested_boolean_array_name =
        named_argument(&mut program, "flag_rows", nested_boolean_array_parameter);
    let exact_nested_boolean_array_literal =
        nested_boolean_array_literal(&mut program, &[&[true, false], &[false, true]]);
    let row_drifted_boolean_array_literal =
        nested_boolean_array_literal(&mut program, &[&[true, false], &[true, false]]);
    let flattened_boolean_array_literal =
        boolean_array_literal(&mut program, &[true, false, false, true]);
    let nested_boolean_array_value = vec![ProofValueSubstitution::nested_boolean_array(
        nested_boolean_array_parameter,
        &[Arc::from(&[true, false][..]), Arc::from(&[false, true][..])],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_boolean_array_name),
        &ProofFact::Expression(exact_nested_boolean_array_literal),
        context(&nested_boolean_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(nested_boolean_array_name),
        &ProofFact::Expression(row_drifted_boolean_array_literal),
        context(&nested_boolean_array_value),
        context(&no_values),
    ));
    assert!(
        !proof_facts_match(
            &program,
            &ProofFact::Expression(nested_boolean_array_name),
            &ProofFact::Expression(flattened_boolean_array_literal),
            context(&nested_boolean_array_value),
            context(&no_values),
        ),
        "container-delimited traces distinguish nested and flat arrays with identical leaves"
    );

    let tensor_parameter = symbol(964);
    let tensor_name = named_argument(&mut program, "flag_planes", tensor_parameter);
    let tensor_planes = vec![
        vec![vec![true, false], vec![false, true]],
        vec![vec![true, true], vec![false, false]],
    ];
    let exact_tensor_literal = boolean_tensor3_literal(&mut program, tensor_planes.clone());
    let regrouped_tensor_literal = boolean_tensor3_literal(
        &mut program,
        vec![
            vec![tensor_planes[0][0].clone()],
            vec![
                tensor_planes[0][1].clone(),
                tensor_planes[1][0].clone(),
                tensor_planes[1][1].clone(),
            ],
        ],
    );
    let tensor_as_matrix_literal = nested_boolean_array_literal(
        &mut program,
        &[
            &tensor_planes[0][0],
            &tensor_planes[0][1],
            &tensor_planes[1][0],
            &tensor_planes[1][1],
        ],
    );
    let tensor_as_flat_literal = boolean_array_literal(
        &mut program,
        &[true, false, false, true, true, true, false, false],
    );
    let tensor_value = vec![ProofValueSubstitution::boolean_tensor3(
        tensor_parameter,
        &[
            Arc::from(vec![
                Arc::from(&[true, false][..]),
                Arc::from(&[false, true][..]),
            ]),
            Arc::from(vec![
                Arc::from(&[true, true][..]),
                Arc::from(&[false, false][..]),
            ]),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(tensor_name),
        &ProofFact::Expression(exact_tensor_literal),
        context(&tensor_value),
        context(&no_values),
    ));
    for drifted in [
        regrouped_tensor_literal,
        tensor_as_matrix_literal,
        tensor_as_flat_literal,
    ] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(tensor_name),
            &ProofFact::Expression(drifted),
            context(&tensor_value),
            context(&no_values),
        ));
    }
}

#[test]
fn membership_identity_retains_indices_and_rejects_unapplied_static_bindings() {
    use crate::proof_contracts::quotients::relation_plan::proof_fact_identity::{
        ProofFactIdentityContext, proof_facts_match,
    };
    let mut program = TypedTrees::default();
    let value = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let mut argument = |name: &str, argument_symbol| {
        let handle = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: argument_symbol,
                name: Identifier::generated(name),
            });
        program
            .type_reference_table
            .insert_type_reference_handles(vec![handle])
    };
    let seven = argument("7", SymbolHandle::invalid());
    let same_seven = argument("7", SymbolHandle::invalid());
    let nine = argument("9", SymbolHandle::invalid());
    let parameter = symbol(995);
    let open = argument("Index", parameter);
    let membership = |arguments| {
        ProofFact::Membership(ProofMembershipFact {
            value,
            domain_symbol: symbol(996),
            domain_arguments: arguments,
            ..Default::default()
        })
    };
    let bindings = [super::RepresentativeStaticBinding {
        parameter,
        kind: super::RepresentativeStaticBindingKind::Const,
        argument: static_argument("7"),
    }];
    let context = |static_bindings| ProofFactIdentityContext {
        values: &[],
        static_bindings,
    };
    assert!(proof_facts_match(
        &program,
        &membership(seven),
        &membership(same_seven),
        context(&[]),
        context(&bindings)
    ));
    assert!(!proof_facts_match(
        &program,
        &membership(seven),
        &membership(nine),
        context(&[]),
        context(&[])
    ));
    // Even identical source binders do not establish equality after different
    // application contexts; this path has not substituted those arguments.
    assert!(!proof_facts_match(
        &program,
        &membership(open),
        &membership(open),
        context(&bindings),
        context(&[])
    ));
}

#[test]
fn proof_fact_recursive_primitive_arrays_retain_every_container_and_leaf_landing() {
    use crate::proof_contracts::quotients::relation_plan::proof_fact_identity::{
        ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
    };
    use crate::proof_contracts::quotients::relation_plan::runtime_correspondence::{
        ClosedFloatArrayElement, ClosedIntegerArrayElement, ClosedRecursiveArrayElement as Value,
    };

    let mut program = TypedTrees::default();
    let no_values = Vec::new();
    let context = |values| ProofFactIdentityContext {
        values,
        static_bindings: &[],
    };

    let boolean_parameter = symbol(1005);
    let boolean_name = named_argument(&mut program, "deep_flags", boolean_parameter);
    let tensor = boolean_tensor3_literal(
        &mut program,
        vec![vec![vec![true, false], vec![false, true]]],
    );
    let exact_boolean = wrap_array_literal(&mut program, [tensor]);
    let matrix = nested_boolean_array_literal(&mut program, &[&[true, false], &[false, true]]);
    let flat = boolean_array_literal(&mut program, &[true, false, false, true]);
    let boolean_value = vec![ProofValueSubstitution::recursive_primitive_array(
        boolean_parameter,
        &[Value::Array(Arc::from(vec![Value::Array(Arc::from(
            vec![
                Value::Array(Arc::from(vec![Value::Boolean(true), Value::Boolean(false)])),
                Value::Array(Arc::from(vec![Value::Boolean(false), Value::Boolean(true)])),
            ],
        ))]))],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_name),
        &ProofFact::Expression(exact_boolean),
        context(&boolean_value),
        context(&no_values),
    ));
    for collapsed in [tensor, matrix, flat] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(boolean_name),
            &ProofFact::Expression(collapsed),
            context(&boolean_value),
            context(&no_values),
        ));
    }

    let integer_parameter = symbol(1006);
    let integer_name = named_argument(&mut program, "deep_offsets", integer_parameter);
    let exact_integer_landing = IntegerLanding {
        landed_type: LandedIntegerType::I16,
        domain: ArithmeticDomain::Exact,
    };
    let exact_integer_matrix = nested_integer_array_literal(
        &mut program,
        vec![vec![
            IntegerLiteral::from_value(7).with_landing(exact_integer_landing),
        ]],
    );
    let exact_integer_tensor = wrap_array_literal(&mut program, [exact_integer_matrix]);
    let wrapping_integer_matrix = nested_integer_array_literal(
        &mut program,
        vec![vec![IntegerLiteral::from_value(7).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::I16,
                domain: ArithmeticDomain::Wrapping,
            },
        )]],
    );
    let wrapping_integer_tensor = wrap_array_literal(&mut program, [wrapping_integer_matrix]);
    let integer_value = vec![ProofValueSubstitution::recursive_primitive_array(
        integer_parameter,
        &[Value::Array(Arc::from(vec![Value::Array(Arc::from(
            vec![Value::Integer(ClosedIntegerArrayElement {
                spelling: "7".to_owned(),
                landing: exact_integer_landing,
            })],
        ))]))],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(integer_name),
        &ProofFact::Expression(exact_integer_tensor),
        context(&integer_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(integer_name),
        &ProofFact::Expression(wrapping_integer_tensor),
        context(&integer_value),
        context(&no_values),
    ));

    let float_parameter = symbol(1007);
    let float_name = named_argument(&mut program, "deep_scales", float_parameter);
    let exact_float_matrix = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
        ]],
    );
    let exact_float_tensor = wrap_array_literal(&mut program, [exact_float_matrix]);
    let drifted_float_matrix = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
        ]],
    );
    let drifted_float_tensor = wrap_array_literal(&mut program, [drifted_float_matrix]);
    let float_value = vec![ProofValueSubstitution::recursive_primitive_array(
        float_parameter,
        &[Value::Array(Arc::from(vec![Value::Array(Arc::from(
            vec![Value::Float(ClosedFloatArrayElement {
                spelling: "1.25".to_owned(),
                landing: FloatFormat::F32,
            })],
        ))]))],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(exact_float_tensor),
        context(&float_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(drifted_float_tensor),
        context(&float_value),
        context(&no_values),
    ));
}
