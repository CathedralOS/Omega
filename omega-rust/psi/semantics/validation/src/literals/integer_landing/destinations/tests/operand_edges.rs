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
fn composed_integer_results_supply_the_actual_peer_destination() {
    for peer in [
        "(8u64 | 16u64)",
        "(8u64 + 16u64)",
        "(8u64 + (16u64 | 1u64))",
        "(~8u64)",
        "(8u64 << 1u32)",
        "(match subject { true -> 8u64 + 16u64, false -> 1u64 | 8u64 })",
    ] {
        for anonymous in [
            LARGE_ARGUMENT.to_owned(),
            format!("match subject {{ true -> {LARGE_ARGUMENT}, false -> 7 / 2 * 2 }}"),
        ] {
            for expression in [
                format!("({anonymous}) | {peer}"),
                format!("{peer} | ({anonymous})"),
            ] {
                let source = format!("machine result(subject: bool) -> u64 {{ {expression} }}");
                assert_eq!(width_grants(&typed(&source)).len(), 2, "{source}");
            }
        }
    }
}

#[test]
fn retained_large_integer_payloads_keep_their_own_carrier() {
    for expression in [
        "18446744073709551615u64",
        "~18446744073709551615u64",
        "18446744073709551615u64 | 0u64",
        "18446744073709551615u64 as u64",
        "accept(18446744073709551615u64)",
    ] {
        let source = format!(
            "machine accept(value: u64) -> u64 {{ value }} machine result() -> u64 {{ {expression} }}"
        );
        let program = typed(&source);
        assert_eq!(width_grants(&program).len(), 1, "{source}");
        assert!(
            anonymous_integer_landing_warnings(&program).is_empty(),
            "{source}"
        );
    }
    for expression in [
        "18446744073709551616u64",
        "18446744073709551615u32",
        "18446744073709551615i64",
        "18446744073709551615u64 as u32",
        "18446744073709551615u64 as i64",
        "18446744073709551615u64 as f64",
    ] {
        let source = format!("machine result() -> u64 {{ {expression} }}");
        assert!(width_grants(&typed(&source)).is_empty(), "{source}");
    }
}

#[test]
fn boolean_results_and_untyped_operations_do_not_supply_integer_destinations() {
    for peer in [
        "(8u64 == 8u64)",
        "(8u64 < 16u64)",
        "(!false)",
        "(true && false)",
        "(8 + 16)",
    ] {
        let source = format!("machine result() -> u64 {{ ({LARGE_ARGUMENT}) | {peer} }}");
        assert!(width_grants(&typed(&source)).is_empty(), "{source}");
    }
}

#[test]
fn an_authored_heterogeneous_result_cannot_borrow_its_operand_carrier() {
    let source = format!(
        "operator + u64::authored(left: u64, right: u64) -> bool;
         machine result(value: u64) -> u64 {{ ({LARGE_ARGUMENT}) | (value + value) }}"
    );
    assert!(width_grants(&typed(&source)).is_empty());
}

#[test]
fn computed_result_queries_follow_boolean_and_float_signatures() {
    use typed_trees::statement::StatementNode;
    use typed_trees::types::PrimitiveType;
    for (parameters, destination, expression, expected) in [
        ("value: u64", "bool", "value < 8u64", PrimitiveType::Bool),
        ("value: u64", "bool", "value == 8u64", PrimitiveType::Bool),
        ("value: bool", "bool", "!value && true", PrimitiveType::Bool),
        ("value: f64", "f64", "value + 1.0", PrimitiveType::F64),
        ("value: f32", "f32", "value * 2.0", PrimitiveType::F32),
    ] {
        let source = format!("machine result({parameters}) -> {destination} {{ {expression} }}");
        let program = typed(&source);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let [StatementNode::Expression(result)] =
            program.statement_table.statements(state.statement_nodes)
        else {
            panic!("result expression");
        };
        let actual = crate::expression_types::expression_result_type_reference(
            &program, machine, state, *result,
        )
        .and_then(|reference| program.primitive_type_reference(reference));
        assert_eq!(actual, Some(expected), "{source}");
    }
}

#[test]
fn computed_result_queries_accept_shared_children_but_not_cycles() {
    use typed_trees::statement::StatementNode;
    let returned = |program: &TypedTrees| {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let [StatementNode::Expression(result)] =
            program.statement_table.statements(state.statement_nodes)
        else {
            panic!("result expression");
        };
        *result
    };
    let query = |program: &TypedTrees| {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        crate::expression_types::expression_result_type_reference(
            program,
            machine,
            state,
            returned(program),
        )
    };
    let program = typed("machine result() -> u64 { (8u64 | 16u64) | (1u64 | 2u64) }");
    let root = returned(&program);
    assert!(query(&program).is_some());
    let mut shared = program.clone();
    let ExpressionNode::Binary(binary) = shared.expression_table.expression_mut(root) else {
        panic!("binary result");
    };
    binary.right = binary.left;
    assert!(query(&shared).is_some());
    for right in [root, ExpressionHandle::invalid()] {
        let mut invalid = program.clone();
        let ExpressionNode::Binary(binary) = invalid.expression_table.expression_mut(root) else {
            panic!("binary result");
        };
        binary.right = right;
        assert!(query(&invalid).is_none());
    }
    let mut unary = typed("machine result() -> u64 { ~8u64 }");
    let root = returned(&unary);
    let ExpressionNode::Unary(operation) = unary.expression_table.expression_mut(root) else {
        panic!("unary result");
    };
    operation.operand = root;
    assert!(query(&unary).is_none());
}

#[test]
fn computed_result_queries_do_not_copy_refinements_or_erase_policy() {
    use typed_trees::statement::StatementNode;
    use typed_trees::types::TypeReferenceNode;

    for (parameter_type, expression) in [
        ("u64 [0..=10]", "value + 1u64"),
        ("u64 in Wrapping", "value + 1"),
        ("u64 in Saturating", "value + 1"),
        ("u64 in Trapping", "value + 1"),
        ("u64", "(value as u64 in Wrapping) + 1"),
    ] {
        let source = format!("machine result(value: {parameter_type}) -> u64 {{ {expression} }}");
        let program = typed(&source);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let [StatementNode::Expression(result)] =
            program.statement_table.statements(state.statement_nodes)
        else {
            panic!("result expression");
        };
        let result = crate::expression_types::expression_result_type_reference(
            &program, machine, state, *result,
        );
        // Unresolved is permitted until qualified results are retained. A
        // discovered type must not claim an operand range or erase its policy.
        if let Some(result) = result {
            if parameter_type.contains("[0..=10]") {
                assert!(
                    !matches!(
                        program.type_reference_table.type_reference(result),
                        TypeReferenceNode::Constrained { .. }
                    ),
                    "{source}"
                );
            } else {
                let expected = if expression.contains("Wrapping") {
                    numerics::arithmetic::ArithmeticDomain::Wrapping
                } else {
                    program.arithmetic_domain_for_type_reference(
                        program.state_parameters(state)[0].type_reference,
                    )
                };
                assert_eq!(
                    program.arithmetic_domain_for_type_reference(result),
                    expected,
                    "{source}"
                );
            }
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
    check_shared_operand_custody(LARGE_ARGUMENT, 2);
    check_shared_operand_custody("18446744073709551615u64", 1);
}

fn check_shared_operand_custody(value: &str, expected_grants: usize) {
    let source = format!(
        "machine result() -> u64 {{ let saved: u64 = ({value}) | 8u64; let floating: f64 = 0u64 as f64; saved << 0u64 }}"
    );
    let program = typed(&source);
    assert_eq!(width_grants(&program).len(), expected_grants);
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
