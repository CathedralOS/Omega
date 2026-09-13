use super::*;
use typed_trees::statement::StatementNode;

mod fields;
mod selected_meaning;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn closed_record_fields_share_exact_bounds_and_keep_projection_custody() {
    let original = typed(
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
        crate::arithmetic_domains::validate_arithmetic_domains(
            program,
            machine,
            Some(state),
            expression,
            &ValueEnv::default(),
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
    let point = crate::literals::closed_record_integer_projection(&original, projection).unwrap();
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
        assert!(crate::literals::closed_record_integer_projection(&changed, projection).is_none());
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
    assert!(crate::literals::closed_record_integer_projection(&changed, projection).is_none());
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
    let program = typed(
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
    crate::arithmetic_domains::validate_arithmetic_domains(
        &program,
        machine,
        Some(state),
        expression,
        &ValueEnv::default(),
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

    let program = typed(
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
    let point = crate::literals::closed_record_integer_projection(&program, expression).unwrap();
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
        let program = typed(&format!("machine value() -> u64 {{ {expression} }}"));
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
    let program = typed("machine value(input: u64[2..=2]) -> u64 { input + 1 }");
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

#[test]
fn enforced_type_bounds_require_exact_owned_bounded_integer_carriers() {
    for (field_type, expected) in [
        ("u64 [0..=5]", Some((0, 5))),
        ("u8", Some((0, 255))),
        ("i16 [-2..=5]", Some((-2, 5))),
        ("u8 [0..=1000]", Some((0, 255))),
        ("u64", None),
        ("&u64 [0..=5]", None),
        ("&mut u64 [0..=5]", None),
        ("u64 [0..=5] in Wrapping", None),
        ("u64 [0..=5] in Saturating", None),
        ("u64 [0..=5] in Trapping", None),
        ("AtomicU32", None),
        ("f64", None),
        ("bool", None),
    ] {
        let program = typed(&format!("data Counter {{ remaining: {field_type}; }}"));
        let data = &program.data_definitions()[0];
        let typed_trees::data::DataMember::Field(field) = &program.data_members(data)[0] else {
            panic!("declared field");
        };
        assert_eq!(
            enforced_integer_type_bounds(&program, field.type_reference),
            expected,
            "{field_type}"
        );
    }
}

#[test]
fn enforced_type_bounds_reject_a_same_spelled_nominal_impostor() {
    let mut program = typed("data Impostor {}");
    let symbol = program.data_definitions()[0].symbol;
    let handle = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: typed_trees::name::Identifier::generated_static("u8"),
        });
    assert_eq!(
        program.primitive_type_reference(handle),
        Some(PrimitiveType::U8)
    );
    assert_eq!(enforced_integer_type_bounds(&program, handle), None);
}

fn query(source: &str) -> Option<(i64, i64)> {
    let program = typed(source);
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
fn immutable_bounds_do_not_select_carrier_width_by_spelling() {
    let mut program = typed("machine read(input: u64) -> u64 { input }");
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let reference = program.state_parameters(&state)[0].type_reference;
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        panic!("builtin carrier");
    };
    let symbol = *symbol;
    let forged = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: typed_trees::name::Identifier::generated_static("u8"),
        });
    program.state_parameters.span_mut_or_empty(state.parameters)[0].type_reference = forged;
    let typed_trees::statement::StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("input expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, &machine, &state, expression),
        None,
        "the actual u64 ceiling does not fit the signed interval engine"
    );
}

#[test]
fn immutable_bounds_exclude_the_signed_remainder_overflow_pair() {
    for carrier in ["i8", "i16", "i32", "i64"] {
        assert_eq!(
            query(&format!(
                "machine value(input: {carrier}) -> {carrier} {{ input % -1{carrier} }}"
            )),
            None,
            "{carrier} MIN % -1 is not an Exact value"
        );
        assert_eq!(
            query(&format!(
                "machine value(input: {carrier} [0..=5]) -> {carrier} {{ input % -1{carrier} }}"
            )),
            Some((0, 0)),
            "ordinary negative divisors remain supported"
        );
    }
}

#[test]
fn bounded_result_does_not_excuse_an_overflowing_unsigned_intermediate() {
    assert_eq!(
        query("machine value(input: u64) -> u64 { (input + 1) % 5 + 6 }"),
        None
    );
    assert_eq!(
        query("machine value(input: u64) -> u64 { (input + 0) % 5 + 6 }"),
        Some((6, 10))
    );
}

#[test]
fn immutable_arithmetic_keeps_operand_landing_obligations() {
    for source in [
        "machine value(input: u8) -> u8 { input % 256 }",
        "machine value(input: u64) -> u64 { input % -1 }",
        "machine value(input: u8 [0..=5]) -> u8 { input + 1u64 }",
    ] {
        assert_eq!(query(source), None, "{source}");
    }
    assert_eq!(
        query("machine value(input: u8 [0..=5]) -> u8 { input + 1u8 }"),
        Some((1, 6))
    );
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
fn computed_bounds_retain_carrier_identity_without_input_refinements() {
    let program = typed("machine value(input: u64 [20..=30]) -> u64 { input % 5 }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("computed expression");
    };
    let value =
        bounds(&program, machine.symbol, Some(state), expression).expect("builtin remainder");
    let reference = value.type_reference.expect("the result remains typed");
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        panic!("operand refinements must not become result facts");
    };
    assert_eq!(
        program.symbols.builtin_type_atom(*symbol),
        Some(symbols::BuiltinTypeAtom::U64)
    );
    assert_eq!(
        value.interval,
        Interval {
            low: Some(0),
            high: Some(4)
        }
    );
}

#[test]
fn immutable_arithmetic_uses_existing_interval_transfers() {
    for (expression, expected) in [
        ("input % 256u16", (0, 255)),
        ("(input % 128u16) + 1u16", (1, 128)),
        ("(input % 128u16) * 2u16", (0, 254)),
    ] {
        assert_eq!(
            query(&format!(
                "machine value(input: u16) -> u16 {{ {expression} }}"
            )),
            Some(expected)
        );
    }
    assert_eq!(
        query("machine value(input: u16 [0..=255]) -> u16 { input }"),
        Some((0, 255))
    );
    assert_eq!(
        query(
            "machine value(input: i16 [1..=10], divisor: i16 [-2..=-1]) -> i16 { input / divisor }"
        ),
        Some((-10, 0))
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
fn another_states_same_spelled_parameter_has_no_bound() {
    let program = typed(
        "machine first(input: u16) -> u16 { input % 256u16 } machine second(input: u16) -> u16 { input }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let typed_trees::statement::StatementNode::Expression(expression) = program
        .statement_table
        .statements(first_state.statement_nodes)[0]
    else {
        panic!("expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, second, second_state, expression),
        None
    );
}

#[test]
fn comparison_bounds_keep_unsigned_floors_without_representable_ceilings() {
    let program = typed("machine compare(left: u64, right: u64) -> bool { left > right }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("comparison expression");
    };
    let (left, right) = builtin_comparison_intervals(&program, machine, state, expression)
        .expect("both exact immutable unsigned parameters");
    assert_eq!(
        left,
        Interval {
            low: Some(0),
            high: None
        }
    );
    assert_eq!(right, left);
}

#[test]
fn comparison_bounds_do_not_rebind_a_foreign_parameter_by_spelling() {
    let program = typed(
        "machine first(left: u64, right: u64) -> bool { left > right }
        machine second(left: u64, right: u64) -> bool { left > right }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let typed_trees::statement::StatementNode::Expression(expression) = program
        .statement_table
        .statements(first_state.statement_nodes)[0]
    else {
        panic!("comparison expression");
    };
    assert!(builtin_comparison_intervals(&program, second, second_state, expression).is_none());
}
