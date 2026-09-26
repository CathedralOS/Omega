use super::{
    arithmetic_implication_fixture, baseline_arithmetic_implication_fixture, binary_expression,
    call_with_arguments, direct_lift_implication_fixture, fixed_call_fixture, named_argument,
    symbol,
};
use crate::validation::proof_contracts::quotients::relation_plan::correspondence_certificate::{
    DirectLiftPreconditionProof, FixedRepresentativeCallPreconditions,
    FixedRepresentativeCallProof, QuotientCorrespondenceEvidence,
    compose_lift_correspondence_certificate, derive_fixed_representative_call_preconditions,
};
use crate::validation::proof_contracts::quotients::relation_plan::theorem_schema::TheoremApplicationSide;
use crate::validation::proof_contracts::quotients::relation_plan::{
    RelationPlanError, derive_direct_lift_precondition_implication,
};
use arena::HandleSpan;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::IntegerLiteral;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::domain::{ProofFact, ProofMembershipFact};
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    BinaryOperator, ExpressionNode, TableNamePath,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier;

#[test]
fn fixed_representative_call_exact_match_is_one_runtime_proof_with_both_theorem_sides() {
    let fixture = fixed_call_fixture(false, |program, public, representative, _| {
        let public = named_argument(program, "fixed", public);
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let q = binary_expression(program, public, BinaryOperator::Greater, zero);
        let representative = named_argument(program, "fixed", representative);
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let p = binary_expression(program, representative, BinaryOperator::Greater, zero);
        (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
    });
    assert!(fixture.public_partition.dependent.is_empty());
    assert_eq!(fixture.public_partition.fixed.len(), 1);
    assert!(fixture.representative_partition.dependent.is_empty());
    assert_eq!(fixture.representative_partition.fixed.len(), 1);

    let fixed = derive_fixed_representative_call_preconditions(
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
    .expect("one exact fixed-Q proof discharges the singular representative call");
    assert_eq!(fixed.rows.len(), 1);
    let FixedRepresentativeCallProof::ExactMatch { public } = fixed.rows[0].proof else {
        panic!("identical direct-name fixed facts must retain exact-match priority")
    };
    assert_eq!(public, fixture.public_partition.fixed[0]);
    assert_ne!(fixed.rows[0].theorem_left, fixed.rows[0].theorem_right);

    let dependent = crate::validation::proof_contracts::quotients::relation_plan::correspondence_certificate::DirectLiftPreconditionImplication { rows: Vec::new() };
    let certificate = compose_lift_correspondence_certificate(
        &Ok(fixture.verified_theorem.clone()),
        &fixture.runtime,
        &dependent,
        &fixed,
        &fixture.representative_partition,
    )
    .expect("the fixed-call rows join the bounded lift certificate");
    let QuotientCorrespondenceEvidence::DirectLift {
        fixed: retained, ..
    } = certificate.evidence
    else {
        panic!("direct lift certificate")
    };
    assert_eq!(retained, fixed);
    assert!(
        compose_lift_correspondence_certificate(
            &Ok(fixture.verified_theorem.clone()),
            &fixture.runtime,
            &dependent,
            &FixedRepresentativeCallPreconditions { rows: Vec::new() },
            &fixture.representative_partition,
        )
        .is_none(),
        "a certificate cannot omit one fixed representative call obligation",
    );
}

#[test]
fn fixed_representative_call_strict_arithmetic_retains_full_fixed_q_and_literals() {
    let direct = fixed_call_fixture(false, |program, public, representative, static_symbol| {
        let public_value = named_argument(program, "fixed", public);
        let two = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let lower = binary_expression(program, public_value, BinaryOperator::Greater, two);
        let public_value = named_argument(program, "fixed", public);
        let ten = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(10)));
        let upper = binary_expression(program, public_value, BinaryOperator::LessOrEqual, ten);
        let representative = named_argument(program, "fixed", representative);
        let static_value = named_argument(program, "K", static_symbol);
        let adjusted =
            binary_expression(program, representative, BinaryOperator::Add, static_value);
        let two = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let goal = binary_expression(program, adjusted, BinaryOperator::Greater, two);
        (
            vec![ProofFact::Expression(lower), ProofFact::Expression(upper)],
            ProofFact::Expression(goal),
        )
    });
    let proof = derive_fixed_representative_call_preconditions(
        &direct.program,
        &direct.public_machine,
        &direct.public_state,
        &direct.representative,
        &direct.public_partition,
        &direct.representative_partition,
        &direct.runtime,
        &direct.expected_theorem,
        &direct.verified_theorem,
    )
    .expect("fixed > 2 entails fixed + 1 > 2");
    let FixedRepresentativeCallProof::ArithmeticEntailment { premises } = &proof.rows[0].proof
    else {
        panic!("non-identical fixed facts require strict arithmetic evidence")
    };
    assert_eq!(premises, &direct.public_partition.fixed);
    assert_eq!(premises.len(), 2);

    let literal = fixed_call_fixture(true, |program, _, representative, static_symbol| {
        let three = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
        let two = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let q = binary_expression(program, three, BinaryOperator::Greater, two);
        let representative = named_argument(program, "fixed", representative);
        let static_value = named_argument(program, "K", static_symbol);
        let adjusted =
            binary_expression(program, representative, BinaryOperator::Add, static_value);
        let two = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let p = binary_expression(program, adjusted, BinaryOperator::Greater, two);
        (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
    });
    assert!(
        derive_fixed_representative_call_preconditions(
            &literal.program,
            &literal.public_machine,
            &literal.public_state,
            &literal.representative,
            &literal.public_partition,
            &literal.representative_partition,
            &literal.runtime,
            &literal.expected_theorem,
            &literal.verified_theorem,
        )
        .is_ok(),
        "the exact i32 literal and static const discharge a closed fixed call fact"
    );
}

#[test]
fn fixed_representative_call_rejects_unknown_refuted_mixed_and_identity_drift() {
    for (operator, bound) in [
        (BinaryOperator::Greater, 5),
        (BinaryOperator::LessOrEqual, 2),
    ] {
        let fixture = fixed_call_fixture(false, |program, public, representative, _| {
            let public = named_argument(program, "fixed", public);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let q = binary_expression(program, public, BinaryOperator::Greater, two);
            let representative = named_argument(program, "fixed", representative);
            let bound = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(bound)));
            let p = binary_expression(program, representative, operator, bound);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        });
        assert_eq!(
            derive_fixed_representative_call_preconditions(
                &fixture.program,
                &fixture.public_machine,
                &fixture.public_state,
                &fixture.representative,
                &fixture.public_partition,
                &fixture.representative_partition,
                &fixture.runtime,
                &fixture.expected_theorem,
                &fixture.verified_theorem,
            ),
            Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(0)),
        );
    }

    let mixed = fixed_call_fixture(false, |program, public, representative, _| {
        let public_value = named_argument(program, "fixed", public);
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let q = binary_expression(program, public_value, BinaryOperator::Greater, zero);
        let membership_value = named_argument(program, "fixed", public);
        let representative = named_argument(program, "fixed", representative);
        let minus_one = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(-1)));
        let p = binary_expression(program, representative, BinaryOperator::Greater, minus_one);
        (
            vec![
                ProofFact::Expression(q),
                ProofFact::Membership(ProofMembershipFact {
                    value: membership_value,
                    domain: HandleSpan::empty(),
                    domain_symbol: symbol(1140),
                    authored_domain_selection: None,
                    ..Default::default()
                }),
            ],
            ProofFact::Expression(p),
        )
    });
    assert_eq!(
        derive_fixed_representative_call_preconditions(
            &mixed.program,
            &mixed.public_machine,
            &mixed.public_state,
            &mixed.representative,
            &mixed.public_partition,
            &mixed.representative_partition,
            &mixed.runtime,
            &mixed.expected_theorem,
            &mixed.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(0)),
    );

    let exact = fixed_call_fixture(false, |program, public, representative, _| {
        let public = named_argument(program, "fixed", public);
        let representative = named_argument(program, "fixed", representative);
        (
            vec![ProofFact::Expression(public)],
            ProofFact::Expression(representative),
        )
    });
    let mut missing_side = exact.verified_theorem.clone();
    missing_side.legality_premises.pop();
    assert_eq!(
        derive_fixed_representative_call_preconditions(
            &exact.program,
            &exact.public_machine,
            &exact.public_state,
            &exact.representative,
            &exact.public_partition,
            &exact.representative_partition,
            &exact.runtime,
            &exact.expected_theorem,
            &missing_side,
        ),
        Err(RelationPlanError::DirectLiftFixedTheoremLegalityMismatch),
    );
}

#[test]
fn fixed_representative_call_rejects_member_proof_view_and_literal_domain_drift() {
    let member = fixed_call_fixture(false, |program, public, representative, _| {
        let public = named_argument(program, "fixed", public);
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let q = binary_expression(program, public, BinaryOperator::Greater, zero);
        let mut members = HandleSpan::empty();
        for name in ["fixed", "member"] {
            program
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated_static(name));
        }
        let view = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                head_symbol: representative,
                symbol: representative,
                ..Default::default()
            }));
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let p = binary_expression(program, view, BinaryOperator::Greater, zero);
        (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
    });
    assert_eq!(
        derive_fixed_representative_call_preconditions(
            &member.program,
            &member.public_machine,
            &member.public_state,
            &member.representative,
            &member.public_partition,
            &member.representative_partition,
            &member.runtime,
            &member.expected_theorem,
            &member.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(0)),
    );

    let proof_view = fixed_call_fixture(false, |program, public, representative, _| {
        let public = named_argument(program, "fixed", public);
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let q = binary_expression(program, public, BinaryOperator::Greater, zero);
        let argument = named_argument(program, "fixed", representative);
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
        derive_fixed_representative_call_preconditions(
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
        Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(0)),
    );

    let mut domain = fixed_call_fixture(true, |program, _, representative, _| {
        let three = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
        let two = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let q = binary_expression(program, three, BinaryOperator::Greater, two);
        let representative = named_argument(program, "fixed", representative);
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let p = binary_expression(program, representative, BinaryOperator::Greater, zero);
        (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
    });
    let crate::validation::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::Literal(
        crate::validation::proof_contracts::quotients::relation_plan::runtime_correspondence::ClosedLiftLiteral::Integer { landing, .. },
    ) = &mut domain.runtime.positions[1].source
    else {
        panic!("fixed integer literal fixture")
    };
    landing.domain = ArithmeticDomain::Wrapping;
    assert_eq!(
        derive_fixed_representative_call_preconditions(
            &domain.program,
            &domain.public_machine,
            &domain.public_state,
            &domain.representative,
            &domain.public_partition,
            &domain.representative_partition,
            &domain.runtime,
            &domain.expected_theorem,
            &domain.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(0)),
    );
}

#[test]
fn direct_lift_arithmetic_q_implies_p_retains_full_ordered_q_and_both_sides() {
    let fixture = baseline_arithmetic_implication_fixture();
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
    .expect("x > 2 entails x + 1 > 2 after exact literal/static substitution");
    assert_eq!(implication.rows.len(), 2);
    for (row, side) in implication
        .rows
        .iter()
        .zip([TheoremApplicationSide::Left, TheoremApplicationSide::Right])
    {
        assert_eq!(row.application, side);
        let DirectLiftPreconditionProof::ArithmeticEntailment { premises } = &row.proof else {
            panic!("non-identical integer facts require strict arithmetic evidence")
        };
        assert_eq!(premises, &fixture.public_partition.dependent);
        assert_eq!(premises.len(), 2);
    }
    assert_ne!(implication.rows[0].theorem, implication.rows[1].theorem);

    let exact = direct_lift_implication_fixture(true);
    let exact_implication = derive_direct_lift_precondition_implication(
        &exact.program,
        &exact.public_machine,
        &exact.public_state,
        &exact.representative,
        &exact.public_partition,
        &exact.representative_partition,
        &exact.runtime,
        &exact.expected_theorem,
        &exact.verified_theorem,
    )
    .expect("exact matching remains the priority owner");
    assert!(matches!(
        exact_implication.rows[0].proof,
        DirectLiftPreconditionProof::ExactMatch { .. }
    ));
}

#[test]
fn direct_lift_arithmetic_implication_rejects_unknown_stronger_and_mixed_facts() {
    let stronger = arithmetic_implication_fixture(
        |program, public_symbol, representative_symbol, literal_symbol, static_symbol| {
            let public = named_argument(program, "public", public_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let q = binary_expression(program, public, BinaryOperator::Greater, two);
            let representative = named_argument(program, "representative", representative_symbol);
            let static_value = named_argument(program, "K", static_symbol);
            let left =
                binary_expression(program, representative, BinaryOperator::Add, static_value);
            let literal = named_argument(program, "literal", literal_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let right = binary_expression(program, literal, BinaryOperator::Add, two);
            let p = binary_expression(program, left, BinaryOperator::Greater, right);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &stronger.program,
            &stronger.public_machine,
            &stronger.public_state,
            &stronger.representative,
            &stronger.public_partition,
            &stronger.representative_partition,
            &stronger.runtime,
            &stronger.expected_theorem,
            &stronger.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let refuted = arithmetic_implication_fixture(
        |program, public_symbol, representative_symbol, literal_symbol, _| {
            let public = named_argument(program, "public", public_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let q = binary_expression(program, public, BinaryOperator::Greater, two);
            let representative = named_argument(program, "representative", representative_symbol);
            let literal = named_argument(program, "literal", literal_symbol);
            let p = binary_expression(
                program,
                representative,
                BinaryOperator::LessOrEqual,
                literal,
            );
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &refuted.program,
            &refuted.public_machine,
            &refuted.public_state,
            &refuted.representative,
            &refuted.public_partition,
            &refuted.representative_partition,
            &refuted.runtime,
            &refuted.expected_theorem,
            &refuted.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let mixed = arithmetic_implication_fixture(
        |program, public_symbol, representative_symbol, literal_symbol, _| {
            let public = named_argument(program, "public", public_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let q = binary_expression(program, public, BinaryOperator::Greater, two);
            let member = named_argument(program, "public", public_symbol);
            let representative = named_argument(program, "representative", representative_symbol);
            let literal = named_argument(program, "literal", literal_symbol);
            let p = binary_expression(program, representative, BinaryOperator::Greater, literal);
            (
                vec![
                    ProofFact::Expression(q),
                    ProofFact::Membership(ProofMembershipFact {
                        value: member,
                        domain: HandleSpan::empty(),
                        domain_symbol: symbol(1040),
                        authored_domain_selection: None,
                        ..Default::default()
                    }),
                ],
                ProofFact::Expression(p),
            )
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &mixed.program,
            &mixed.public_machine,
            &mixed.public_state,
            &mixed.representative,
            &mixed.public_partition,
            &mixed.representative_partition,
            &mixed.runtime,
            &mixed.expected_theorem,
            &mixed.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn strict_arithmetic_implication_rejects_member_views_and_conflicting_bindings() {
    use crate::validation::proof_contracts::contract_entailment::{
        StrictArithmeticBindingValue, StrictArithmeticImplicationJudgment,
        StrictArithmeticSymbolBinding, strict_arithmetic_expression_implication,
    };

    let member_path =
        arithmetic_implication_fixture(|program, public_symbol, representative_symbol, _, _| {
            let public = named_argument(program, "public", public_symbol);
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let q = binary_expression(program, public, BinaryOperator::Greater, zero);
            let mut members = HandleSpan::empty();
            for name in ["representative", "member"] {
                program
                    .expression_table
                    .push_name_path_member(&mut members, Identifier::generated_static(name));
            }
            let member = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: representative_symbol,
                    symbol: representative_symbol,
                    ..Default::default()
                }));
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let p = binary_expression(program, member, BinaryOperator::Greater, zero);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        });
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &member_path.program,
            &member_path.public_machine,
            &member_path.public_state,
            &member_path.representative,
            &member_path.public_partition,
            &member_path.representative_partition,
            &member_path.runtime,
            &member_path.expected_theorem,
            &member_path.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let mut program = TypedTrees::default();
    let one_left = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let one_right = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let constant_goal = binary_expression(&mut program, one_left, BinaryOperator::Equal, one_right);
    let conflicting = [
        StrictArithmeticSymbolBinding {
            symbol: symbol(1050),
            value: StrictArithmeticBindingValue::Atom {
                identity: "$a".to_owned(),
                unsigned: false,
            },
        },
        StrictArithmeticSymbolBinding {
            symbol: symbol(1050),
            value: StrictArithmeticBindingValue::Atom {
                identity: "$b".to_owned(),
                unsigned: false,
            },
        },
    ];
    assert_eq!(
        strict_arithmetic_expression_implication(
            &program,
            &Machine::default(),
            &[],
            constant_goal,
            &conflicting,
        ),
        StrictArithmeticImplicationJudgment::Unknown,
    );
}
