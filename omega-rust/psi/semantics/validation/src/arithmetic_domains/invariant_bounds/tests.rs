use super::*;

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
        Some((0, 4))
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
    let value = bounds(&program, machine, state, expression).expect("builtin remainder");
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
