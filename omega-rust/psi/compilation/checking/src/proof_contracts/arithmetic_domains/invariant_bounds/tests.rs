use super::super::ValueEnvironment;
use super::{
    ArithmeticDomain, ExpressionNode, Interval, PrimitiveType, SymbolHandle, TypeReferenceNode,
    TypedTrees, bounds, immutable_integer_expression_bounds,
};
use typed_trees::statement::StatementNode;

mod fields;
mod selected_meaning;

#[test]
fn qualified_record_projection_retains_type_but_does_not_erase_arithmetic_policy() {
    for (field_type, accepted) in [("u64 in Tag", true), ("u64 in Wrapping", false)] {
        let program = crate::front_end::typed_program(&format!(
            "domain u64::Tag;
             data Config [copy] {{ size: {field_type}; }}
             const CONFIG: Config = Config {{ size: 7 }};
             machine read() -> u64 {{ CONFIG.size }}"
        ));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Expression(projection) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("field projection");
        };
        let scalar = crate::closed_record_scalar_projection(&program, projection);
        let integer =
            crate::value_custody::literals::closed_record_integer_projection(&program, projection);
        assert_eq!(scalar.is_some(), accepted, "{field_type}");
        assert_eq!(integer.is_some(), accepted, "{field_type}");
        if let Some(integer) = integer {
            assert_eq!(integer.value.to_u64(), Some(7));
            assert!(matches!(
                program
                    .type_reference_table
                    .type_reference(integer.type_reference.expect("declared field type")),
                TypeReferenceNode::Constrained { .. }
            ));
        }
    }
}

#[test]
fn closed_record_fields_share_exact_bounds_and_keep_projection_custody() {
    let original = crate::front_end::typed_program(
        "data Config [copy] { size: u64; enabled: bool; }
        data Foreign [copy] { size: u64; enabled: bool; }
        const CONFIG: Config = Config { size: 14, enabled: true };
        machine read() -> u64 { CONFIG.size + 7u64 }
        machine effect() -> bool { true }
        machine caller() -> bool { effect() && true }",
    );
    let machine = &original.machines()[0];
    let state = &original.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("arithmetic source");
    };
    let ExpressionNode::Binary(binary) = original.expression_table.expression(expression) else {
        panic!("arithmetic source");
    };
    let projection = binary.left;
    let ExpressionNode::Member(member) = original.expression_table.expression(projection) else {
        panic!("field projection");
    };
    let constructor = member.receiver;
    let ExpressionNode::StructLiteral(literal) = original.expression_table.expression(constructor)
    else {
        panic!("record constructor");
    };
    let sibling = original
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .find(|field| field.name.as_str() == "enabled")
        .unwrap()
        .value;
    let diagnose = |program: &TypedTrees| {
        let mut diagnostics = Vec::new();
        crate::proof_contracts::arithmetic_domains::validate_arithmetic_domains(
            program,
            machine,
            Some(state),
            expression,
            &ValueEnvironment::default(),
            Some(PrimitiveType::U64),
            ArithmeticDomain::Exact,
            "projection arithmetic",
            &mut diagnostics,
        );
        diagnostics
    };
    assert_eq!(
        immutable_integer_expression_bounds(&original, machine, state, expression),
        Some((21, 21))
    );
    assert!(
        diagnose(&original).is_empty(),
        "exact arithmetic uses the closed field point"
    );
    let point =
        crate::value_custody::literals::closed_record_integer_projection(&original, projection)
            .unwrap();
    assert_eq!(point.primitive, Some(PrimitiveType::U64));
    assert_eq!(point.value.to_i64(), Some(14));

    let foreign = &original.data_definitions()[1];
    let typed_trees::data::DataMember::Field(foreign_field) = &original.data_members(foreign)[0]
    else {
        panic!("foreign field");
    };
    for symbol in [SymbolHandle::invalid(), foreign_field.symbol] {
        let mut changed = original.clone();
        let ExpressionNode::Member(member) = changed.expression_table.expression_mut(projection)
        else {
            panic!("projection");
        };
        member.member_symbol = symbol;
        assert!(
            crate::value_custody::literals::closed_record_integer_projection(&changed, projection)
                .is_none()
        );
        assert_eq!(
            immutable_integer_expression_bounds(&changed, machine, state, expression),
            None
        );
        assert!(
            !diagnose(&changed).is_empty(),
            "foreign or missing field cannot supply a point bound"
        );
    }
    let mut changed = original.clone();
    let call = original
        .expression_table
        .iter_expressions()
        .find_map(|(_, node)| matches!(node, ExpressionNode::Call(_)).then_some(node.clone()))
        .unwrap();
    *changed.expression_table.expression_mut(sibling) = call;
    assert!(
        crate::value_custody::literals::closed_record_integer_projection(&changed, projection)
            .is_none()
    );
    assert_eq!(
        immutable_integer_expression_bounds(&changed, machine, state, expression),
        None
    );
    assert!(
        !diagnose(&changed).is_empty(),
        "an unselected call cannot be suppressed by bounds"
    );
}

#[test]
fn projected_integer_landing_keeps_node_overflow_and_full_width_points() {
    let program = crate::front_end::typed_program(
        "data Config [copy] { size: u8; }
        const CONFIG: Config = Config { size: 255 };
        machine read() -> u8 { CONFIG.size + 1 }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("arithmetic source");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, machine, state, expression),
        None
    );
    let mut diagnostics = Vec::new();
    crate::proof_contracts::arithmetic_domains::validate_arithmetic_domains(
        &program,
        machine,
        Some(state),
        expression,
        &ValueEnvironment::default(),
        Some(PrimitiveType::U8),
        ArithmeticDomain::Exact,
        "projection overflow",
        &mut diagnostics,
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("overflow"))
    );

    let program = crate::front_end::typed_program(
        "data Config [copy] { size: u64; }
        const CONFIG: Config = Config { size: 18446744073709551615u64 };
        machine read() -> u64 { CONFIG.size }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("field source");
    };
    let point =
        crate::value_custody::literals::closed_record_integer_projection(&program, expression)
            .unwrap();
    assert_eq!(point.value.to_u64(), Some(u64::MAX));
    assert_eq!(point.primitive, Some(PrimitiveType::U64));
}

#[test]
fn closed_integer_points_retain_width_and_each_typed_operation() {
    for (expression, expected) in [
        ("18446744073709551615u64 % 512u64", Some("511")),
        (
            "9223372036854775808u64 + 256 - 9223372036854775808u64",
            Some("256"),
        ),
        ("9223372036854775808u64 + 1", Some("9223372036854775809")),
        (
            "4294967295u64 * 4294967295u64",
            Some("18446744065119617025"),
        ),
        (
            "18446744073709551615u64 / 3u64",
            Some("6148914691236517205"),
        ),
        ("5i64 % -9223372036854775808i64", Some("5")),
        (
            "-9223372036854775808i64 % -9223372036854775808i64",
            Some("0"),
        ),
        ("7u8 / 2", Some("3")),
        ("1 / 2 * 512", Some("256")),
        ("18446744073709551615u64 + 1 - 1", None),
        ("9223372036854775808u64 * 2 / 2", None),
        ("(0u64 - 1) + 1", None),
        ("0u64 + 18446744073709551616", None),
        ("0u8 + 256", None),
        ("1u64 / 0", None),
        ("1u64 % 0", None),
        ("-9223372036854775808i64 / -1", None),
        ("-9223372036854775808i64 % -1", None),
        ("1u64 + 1i64", None),
        ("0u64 + 1 / 2", None),
    ] {
        let program =
            crate::front_end::typed_program(&format!("machine value() -> u64 {{ {expression} }}"));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let typed_trees::statement::StatementNode::Expression(root) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("closed expression");
        };
        assert_eq!(
            crate::closed_integer_range_bound(&program, root)
                .map(|value| value.to_string())
                .as_deref(),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn wide_points_do_not_grant_landing_or_static_identity_to_variable_operands() {
    for expression in [
        "input / 18446744073709551616",
        "input % 18446744073709551616",
        "18446744073709551616 / input",
        "18446744073709551616 % input",
        "input + (18446744073709551615u64 + 1 - 1)",
    ] {
        assert_eq!(
            query(&format!(
                "machine value(input: u64[1..=2]) -> u64 {{ {expression} }}"
            )),
            None,
            "{expression}"
        );
    }
    let program =
        crate::front_end::typed_program("machine value(input: u64[2..=2]) -> u64 { input + 1 }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::Expression(root) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("variable expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, machine, state, root),
        Some((3, 3))
    );
    assert!(
        crate::closed_integer_range_bound(&program, root).is_none(),
        "a singleton parameter is not a static constant"
    );
}

fn query(source: &str) -> Option<(i64, i64)> {
    let program = crate::front_end::typed_program(source);
    let machine = program.machines().first().unwrap();
    let state = program.machine_states(machine).first().unwrap();
    let typed_trees::statement::StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("value expression");
    };
    immutable_integer_expression_bounds(&program, machine, state, *expression)
}

#[test]
fn unsigned_literal_formation_uses_the_actual_carrier_ceiling() {
    assert_eq!(
        query("machine value() -> u64 { 18446744073709551616u64 % 5u64 }"),
        None
    );
    assert_eq!(
        query("machine value() -> u64 { 18446744073709551615u64 % 5u64 }"),
        Some((0, 0))
    );
}

#[test]
fn unsigned_masks_and_in_width_shifts_bound_their_results() {
    let interval = |source: &str| {
        let program = crate::front_end::typed_program(source);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("expression statement");
        };
        bounds(&program, machine, Some(state), expression, false).map(|value| value.interval)
    };
    let closed = |low, high| Interval {
        low: Some(low),
        high: Some(high),
    };
    assert_eq!(
        interval(
            "machine value(input: u64, depth: u64) -> u64 requires depth <= 3 { (input >> (39 - 9 * depth)) & 511 }"
        ),
        Some(closed(0, 511))
    );
    assert_eq!(
        interval("machine value(input: u16) -> u16 { input >> 4 }"),
        Some(closed(0, 4095))
    );
    // A count that may reach the carrier width, or a signed mask, has no
    // interval here.
    assert_eq!(
        interval("machine value(input: u16, count: u16) -> u16 { input >> count }"),
        None
    );
    assert_eq!(
        interval("machine value(input: i64) -> i64 { input & 511 }"),
        None
    );
}

#[test]
fn mutable_values_initializers_policies_and_oversize_divisors_stay_unknown() {
    for source in [
        "machine value(mut input: u16) -> u16 { input % 256u16 }",
        "machine value(input: u16) -> u16 { let saved: u16 = input; saved % 256u16 }",
        "machine value(input: u16 in Wrapping) -> u16 { input % 256u16 }",
        "machine value(input: i64) -> i64 { input % -9223372036854775808i64 }",
        "machine value(input: u16) -> u16 { input % 0u16 }",
    ] {
        assert_eq!(query(source), None, "{source}");
    }
}

#[test]
fn widening_bounds_do_not_admit_narrowing_signedness_or_policy_erasure() {
    for (source, target, value) in [
        ("u64", "u8", "input as u8"),
        ("i8", "u64", "input as u64"),
        ("u8", "i8", "input as i8"),
        ("u8", "u64 in Wrapping", "input as u64 in Wrapping"),
        ("u8 in Wrapping", "u64", "(input + 1) as u64"),
    ] {
        let program = crate::front_end::typed_program(&format!(
            "machine convert(input: {source}) -> {target} {{ {value} }}"
        ));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("cast expression");
        };
        assert_eq!(
            immutable_integer_expression_bounds(&program, machine, state, expression),
            None,
            "{source}: {value}"
        );
    }
}
