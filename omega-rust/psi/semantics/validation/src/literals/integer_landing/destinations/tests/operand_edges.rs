use super::{LARGE_ARGUMENT, anonymous_integer_landing_warnings, typed, width_grants};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

fn first_binary(program: &TypedTrees, operator: BinaryOperator) -> ExpressionHandle {
    program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Binary(binary) if binary.operator == operator)
                .then_some(handle)
        })
        .expect("fixture binary operation")
}

#[test]
fn builtin_operand_edges_use_retained_peer_carriers_on_either_side() {
    for (parameters, peer) in [
        ("", "8u64"),
        ("value: u64", "value"),
        ("value: &mut u64", "value"),
        ("", "provide()"),
        ("", "(8u32 as u64)"),
        (
            "subject: bool",
            "(match subject { true -> 8u64, false -> 9u64 })",
        ),
    ] {
        for expression in [
            format!("({LARGE_ARGUMENT}) | {peer}"),
            format!("{peer} | ({LARGE_ARGUMENT})"),
        ] {
            let source = format!(
                "machine provide() -> u64 {{ 8u64 }} machine result({parameters}) -> u64 {{ {expression} }}"
            );
            assert_eq!(width_grants(&typed(&source)).len(), 2, "{source}");
        }
    }
}

#[test]
fn match_result_edges_forward_the_typed_operand_destination() {
    let dispatch =
        format!("match subject {{ true -> {LARGE_ARGUMENT}, false -> {LARGE_ARGUMENT} }}");
    for expression in [
        format!("({dispatch}) | 8u64"),
        format!("8u64 | ({dispatch})"),
    ] {
        let source = format!("machine result(subject: bool) -> u64 {{ {expression} }}");
        assert_eq!(width_grants(&typed(&source)).len(), 4, "{source}");
    }
}

#[test]
fn exact_cast_edges_land_whole_anonymous_and_match_results() {
    for expression in [
        LARGE_ARGUMENT.to_owned(),
        format!(
            "match subject {{ true -> {LARGE_ARGUMENT}, false -> match subject {{ _ -> {LARGE_ARGUMENT} }} }}"
        ),
    ] {
        let source = format!("machine result(subject: bool) -> u64 {{ ({expression}) as u64 }}");
        let expected = if expression == LARGE_ARGUMENT { 2 } else { 4 };
        assert_eq!(width_grants(&typed(&source)).len(), expected, "{source}");
    }
}

#[test]
fn operand_and_cast_destinations_retain_fractional_origin_warnings() {
    for expression in [
        "(7 / 2 * 2) | 8u64",
        "8u64 | (7 / 2 * 2)",
        "(7 / 2 * 2) as u64",
    ] {
        let source = format!("machine result() -> u64 {{ {expression} }}");
        let program = typed(&source);
        let warnings = anonymous_integer_landing_warnings(&program);
        assert_eq!(warnings.len(), 1, "{source}: {warnings:?}");
        assert!(warnings[0].message.contains("`7/2`"));
        assert!(warnings[0].message.contains("integer `7`"));
        let origin = first_binary(&program, BinaryOperator::Divide);
        assert_eq!(
            warnings[0].source_span,
            Some(program.expression_table.source_span(origin))
        );
    }
}

#[test]
fn every_match_arm_warns_at_its_operand_or_cast_destination() {
    let dispatch = "match subject { true -> 7 / 2 * 2, false -> 9 / 2 * 2 }";
    for expression in [
        format!("({dispatch}) | 8u64"),
        format!("8u64 | ({dispatch})"),
        format!("({dispatch}) as u64"),
    ] {
        let program = typed(&format!(
            "machine result(subject: bool) -> u64 {{ {expression} }}"
        ));
        let warnings = anonymous_integer_landing_warnings(&program);
        assert_eq!(warnings.len(), 2, "{expression}: {warnings:?}");
        for (fraction, integer) in [("`7/2`", "integer `7`"), ("`9/2`", "integer `9`")] {
            assert!(warnings.iter().any(|warning| {
                warning.message.contains(fraction) && warning.message.contains(integer)
            }));
        }
    }
}

#[test]
fn invalid_or_already_typed_arithmetic_receives_no_anonymous_width_grant() {
    for value in [
        "18446744073709551616 / 3",
        "18446744073709551616",
        "18446744073709551616 / 2u64",
        "18446744073709551616 % 2",
    ] {
        for expression in [format!("({value}) | 8u64"), format!("({value}) as u64")] {
            let source = format!("machine result() -> u64 {{ {expression} }}");
            assert!(width_grants(&typed(&source)).is_empty(), "{source}");
        }
    }
    for value in ["7 / 2", "18446744073709551616", "7u64 / 2 * 2"] {
        for expression in [format!("({value}) | 8u64"), format!("({value}) as u64")] {
            let source = format!("machine result() -> u64 {{ {expression} }}");
            assert!(
                anonymous_integer_landing_warnings(&typed(&source)).is_empty(),
                "{source}"
            );
        }
    }
}

#[test]
fn shift_operands_do_not_acquire_symmetric_peer_destinations() {
    for expression in [
        format!("8u64 << ({LARGE_ARGUMENT})"),
        format!("({LARGE_ARGUMENT}) << 8u64"),
        format!("8u64 >> ({LARGE_ARGUMENT})"),
        format!("({LARGE_ARGUMENT}) >> 8u64"),
    ] {
        let source = format!("machine result() -> u64 {{ {expression} }}");
        assert!(width_grants(&typed(&source)).is_empty(), "{source}");
    }
}

#[test]
fn shared_operand_grants_do_not_escape_to_float_cast_or_shift_count_edges() {
    let source = format!(
        "machine result() -> u64 {{ let saved: u64 = ({LARGE_ARGUMENT}) | 8u64; let floating: f64 = 0u64 as f64; saved << 0u64 }}"
    );
    let program = typed(&source);
    assert_eq!(width_grants(&program).len(), 2);
    let operation = first_binary(&program, BinaryOperator::BitwiseOr);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(operation) else {
        panic!("operand owner");
    };
    let shared = binary.left;
    let cast = program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Cast(_)).then_some(handle))
        .expect("independent float conversion");
    let shift = first_binary(&program, BinaryOperator::ShiftLeft);
    for consumer in [cast, shift] {
        let mut changed = program.clone();
        match changed.expression_table.expression_mut(consumer) {
            ExpressionNode::Cast(cast) => cast.value = shared,
            ExpressionNode::Binary(binary) => binary.right = shared,
            _ => panic!("fixture consumer"),
        }
        assert!(width_grants(&changed).is_empty(), "consumer={consumer:?}");
    }
}

#[test]
fn selected_authored_operator_does_not_supply_a_builtin_peer_grant() {
    use language_core::OperatorSpelling;
    use typed_trees::typed_trees::{
        ClosedConformanceApplication, ClosedConformanceRowIdentity, MachineSpecialization,
    };

    let source = format!(
        "trait SelectedArithmetic {{ operator + add(left: Self, right: Self) -> Self; }}
         Chosen: u64 satisfies SelectedArithmetic {{
             machine add(left: u64, right: u64) -> u64 {{ 100u64 }}
         }}
         machine result(value: u64) -> u64 {{ value + ({LARGE_ARGUMENT}) }}"
    );
    let mut program = typed(&source);
    assert_eq!(width_grants(&program).len(), 2);
    let conformance = program.conformances()[0].clone();
    let rows = program
        .closed_conformance_rows(&conformance)
        .expect("closed selected conformance")
        .iter()
        .map(|row| ClosedConformanceRowIdentity {
            declaring_trait: row.declaring_trait,
            requirement: row.requirement,
            realization_machine: row.realization_machine,
            realization_state: row.realization_state,
        })
        .collect();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "result")
        .expect("operand owner")
        .clone();
    let state = &program.machine_states(&machine)[0];
    let peer_type = program.state_parameters(state)[0].type_reference;
    let application = ClosedConformanceApplication {
        declaration: conformance.symbol,
        subject_identity: Some(program.display_type_reference(peer_type)),
        trait_definition: conformance.trait_symbol,
        rows,
        ..Default::default()
    };
    // This query mutation supplies selected meaning, not publication evidence.
    program.machine_specializations.push(MachineSpecialization {
        template: machine.symbol,
        instance: machine.symbol,
        conformance_arguments: vec![conformance.symbol],
        conformance_applications: vec![application],
        ..Default::default()
    });
    assert_eq!(
        typed_trees::operator::selected_trait_operator_meanings(
            &program,
            machine.symbol,
            OperatorSpelling::Add,
            &[Some(peer_type), None],
        )
        .len(),
        1
    );
    assert!(width_grants(&program).is_empty());
}
