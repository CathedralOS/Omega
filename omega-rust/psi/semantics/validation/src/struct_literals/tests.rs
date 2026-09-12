use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolved source");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed source")
}

fn construction_diagnostics(source: &str) -> Vec<Diagnostic> {
    let program = typed(source);
    let mut diagnostics = Vec::new();
    validate_struct_literal_fields(&program, &mut diagnostics);
    diagnostics
}

#[test]
fn synthesized_payload_tags_are_not_constructor_values() {
    let mut program = typed(
        "trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
         data Message { case Empty; case Data(value: i32); }
         MessageEquatable: Message satisfies Equatable;
         data Envelope { message: Message; }
         EnvelopeEquatable: Envelope satisfies Equatable;
         machine equal(left: Envelope, right: Envelope) -> bool { left == right }",
    );
    let mut diagnostics = Vec::new();
    validate_struct_literal_fields(&program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let generated: Vec<_> = program.expression_table.expression_entries().filter_map(|(expression, node)| {
        matches!(node, ExpressionNode::Binary(binary) if binary.operator == typed_trees::expression::BinaryOperator::CaseMembership)
            .then_some(expression)
    }).collect();
    assert!(!generated.is_empty());
    for expression in generated {
        let ExpressionNode::Binary(binary) = program.expression_table.expression_mut(expression)
        else {
            panic!("generated tag");
        };
        binary.operator = typed_trees::expression::BinaryOperator::Equal;
    }
    validate_struct_literal_fields(&program, &mut diagnostics);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has a payload")),
        "{diagnostics:?}"
    );
}

#[test]
fn array_elements_require_explicit_integer_carrier_conversions() {
    for source in [
        "machine read(input: u8) -> [u16;1] { [input] }",
        "machine read(input: u8) -> [u16;1] { let values: [u16;1] = [input]; values }",
        "machine read(input: u8) -> [[u16;1];1] { [[input]] }",
        "machine helper(input: u8) -> u8 { input } machine read(input: u8) -> [u16;1] { [helper(input)] }",
        "machine read(input: u8) -> [u16;1] { [input ^ 1u8] }",
        "machine read() -> [u8;1] { let value: u16 = 7u16; [value] }",
        "machine read() -> [u8;1] { [7u16] }",
    ] {
        let program = typed(source);
        let diagnostics = crate::validate_program(&program).expect_err(source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("array literal element has type")
                    && diagnostic.message.contains("explicit conversion")
            }),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn array_elements_preserve_anonymous_landing_and_explicit_conversions() {
    for source in [
        "machine read(input: u8) -> [u16;1] { [input as u16] }",
        "machine read(input: u8) -> [u16;1] { let values: [u16;1] = [input as u16]; values }",
        "machine read(input: u8) -> [[u16;1];1] { [[input as u16]] }",
        "machine helper(input: u8) -> u8 { input } machine read(input: u8) -> [u16;1] { [helper(input) as u16] }",
        "machine read() -> [u8;2] { [7, 7u16 as u8] }",
        "machine read(input: u8 [0..=7]) -> [u8;1] { [input] }",
    ] {
        let program = typed(source);
        assert!(crate::validate_program(&program).is_ok(), "{source}");
    }
}

#[test]
fn guarded_owned_field_reconstruction_preserves_declared_range() {
    let diagnostics = construction_diagnostics(
        "data Countdown { remaining: u64 [0..=5]; }
         machine rebuild(countdown: Countdown) -> Countdown {
             transition countdown.remaining > 0 {
                 true -> Countdown { remaining: countdown.remaining - 1 }
                 false -> Countdown { remaining: 0 }
             }
         }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn forwarding_owned_records_preserves_other_operands_constructor_guards() {
    let source = "data Countdown { remaining: u64 [0..=5]; }
        data Payload { value: u64; }
        machine rebuild(countdown: Countdown, payload: Payload) -> Countdown {
            transition countdown.remaining > 0 {
                true -> next(payload, Countdown { remaining: countdown.remaining - 1 })
                false -> Countdown { remaining: 0 }
            }
            state next(forwarded: Payload, result: Countdown) { result }
        }";
    for source in [
        source.to_owned(),
        source
            .replace(
                "next(payload, Countdown { remaining: countdown.remaining - 1 })",
                "next(Countdown { remaining: countdown.remaining - 1 }, payload)",
            )
            .replace(
                "forwarded: Payload, result: Countdown",
                "result: Countdown, forwarded: Payload",
            ),
    ] {
        let diagnostics = construction_diagnostics(&source);
        assert!(diagnostics.is_empty(), "{source}\n{diagnostics:?}");
        for parameter in ["payload: &Payload", "mut payload: Payload"] {
            let changed = source.replace("payload: Payload", parameter);
            let diagnostics = construction_diagnostics(&changed);
            assert!(!diagnostics.is_empty(), "{changed}");
        }
        let changed = format!(
            "machine opaque(value: Payload) -> Payload {{ value }} {}",
            source
                .replace("next(payload,", "next(opaque(payload),")
                .replace("}, payload)", "}, opaque(payload))")
        );
        let diagnostics = construction_diagnostics(&changed);
        assert!(!diagnostics.is_empty(), "{changed}");
    }
}

#[test]
fn field_reconstruction_uses_the_selected_guard_polarity() {
    for (guard, arm, accepted) in [
        ("countdown.remaining > 0", "true", true),
        ("countdown.remaining > 0", "false", false),
        ("countdown.remaining == 0", "false", true),
        ("countdown.remaining == 0", "true", false),
        ("!(countdown.remaining == 0)", "true", true),
        ("!(countdown.remaining > 0)", "true", false),
    ] {
        let diagnostics = construction_diagnostics(&format!(
            "data Countdown {{ remaining: u64 [0..=5]; }}
             machine rebuild(countdown: Countdown) -> Countdown {{
                 transition {guard} {{
                     {arm} -> Countdown {{ remaining: countdown.remaining - 1 }}
                     _ -> Countdown {{ remaining: 0 }}
                 }}
             }}"
        ));
        assert_eq!(
            diagnostics.is_empty(),
            accepted,
            "{guard}, {arm}: {diagnostics:?}"
        );
    }
}

#[test]
fn shared_target_identity_does_not_replace_false_arm_polarity() {
    let mut program = typed(
        "data Countdown { remaining: u64 [0..=5]; }
         machine rebuild(countdown: Countdown) -> Countdown {
             transition countdown.remaining > 0 {
                 true -> Countdown { remaining: countdown.remaining - 1 }
                 false -> Countdown { remaining: 0 }
             }
         }",
    );
    let statements = program.machine_states(&program.machines()[0])[0].statement_nodes;
    let transition = program
        .statement_table
        .statements_mut(statements)
        .iter_mut()
        .find_map(|statement| match statement {
            StatementNode::Transition(transition) => Some(transition),
            _ => None,
        })
        .expect("guarded transition");
    transition.continuation = transition.target;
    let mut diagnostics = Vec::new();
    validate_struct_literal_fields(&program, &mut diagnostics);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("field `remaining` cannot be proven within its declared range")
    }));
}

#[test]
fn field_reconstruction_discards_facts_before_effectful_target_evaluation() {
    for target in [
        "Countdown { scratch: touch(&mut other), remaining: countdown.remaining - 1 }",
        "finish(touch(&mut other), Countdown { remaining: countdown.remaining - 1 })",
        "finish(0, Countdown { scratch: touch(&mut other), remaining: countdown.remaining - 1 })",
    ] {
        let diagnostics = construction_diagnostics(&format!(
            "data Countdown {{ remaining: u64 [0..=5]; scratch: u64; }}
             machine touch(other: &mut Countdown) -> u64 {{ other.remaining = 0; 0 }}
             machine finish(scratch: u64, value: Countdown) -> Countdown {{ value }}
             machine rebuild(countdown: Countdown, mut other: Countdown) -> Countdown {{
                 transition countdown.remaining > 0 {{ true -> {target} _ -> Countdown {{ remaining: 0, scratch: 0 }} }}
             }}"
        ));
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("field `remaining` cannot be proven within its declared range `0..=5`")),
            "{target}: {diagnostics:?}"
        );
    }
}

#[test]
fn field_guard_rejects_nested_reference_and_foreign_same_spelled_subjects() {
    let program = typed(
        "data Countdown { remaining: u64 [0..=5]; }
         data Borrowed { countdown: &Countdown; }
         machine first(countdown: Countdown) -> u64 { countdown.remaining }
         machine second(countdown: Countdown) -> u64 { countdown.remaining }
         machine borrowed(input: Borrowed) -> u64 { input.countdown.remaining }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let borrowed = &program.machines()[2];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let borrowed_state = &program.machine_states(borrowed)[0];
    let expression = |state: &State| {
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("field expression");
        };
        expression
    };
    assert!(guard_bounds::has_immutable_inputs(
        &program,
        first,
        first_state,
        expression(first_state)
    ));
    assert!(!guard_bounds::has_immutable_inputs(
        &program,
        second,
        second_state,
        expression(first_state)
    ));
    assert!(!guard_bounds::has_immutable_inputs(
        &program,
        borrowed,
        borrowed_state,
        expression(borrowed_state)
    ));
}

#[test]
fn field_guard_requires_the_exact_retained_selector() {
    for selector in ["invalid", "foreign", "parameter", "other_field"] {
        let mut program = typed(
            "data Countdown { remaining: u64 [0..=5]; other: u64 [0..=5]; }
             data Foreign { remaining: u64 [0..=5]; }
             machine read(countdown: Countdown) -> u64 { countdown.remaining }",
        );
        let machine = program.machines()[0].clone();
        let state = program.machine_states(&machine)[0].clone();
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("field expression");
        };
        let field_symbol = |data_position, field_position| {
            let data = &program.data_definitions()[data_position];
            let DataMember::Field(field) = &program.data_members(data)[field_position] else {
                panic!("field");
            };
            field.symbol
        };
        let replacement = match selector {
            "foreign" => field_symbol(1, 0),
            "other_field" => field_symbol(0, 1),
            "parameter" => program.state_parameters(&state)[0].symbol,
            _ => symbols::SymbolHandle::invalid(),
        };
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression)
        else {
            panic!("member");
        };
        member.member_symbol = replacement;
        assert!(
            !guard_bounds::has_immutable_inputs(&program, &machine, &state, expression),
            "{selector}"
        );
    }
}

#[test]
fn field_guard_does_not_import_authored_operator_facts() {
    for (operator, guard) in [
        (
            "operator > u64::not_a_bound(left: u64, right: u64) -> bool;",
            "countdown.remaining > 0",
        ),
        (
            "operator - u64::not_a_decrement(left: u64, right: u64) -> u64;",
            "countdown.remaining > 0",
        ),
    ] {
        let diagnostics = construction_diagnostics(&format!(
            "{operator}
             data Countdown {{ remaining: u64 [0..=5]; }}
             machine rebuild(countdown: Countdown) -> Countdown {{
                 transition {guard} {{ true -> Countdown {{ remaining: countdown.remaining - 1 }} _ -> Countdown {{ remaining: 0 }} }}
             }}"
        ));
        assert!(!diagnostics.is_empty(), "{operator}, {guard}");
    }
}

#[test]
fn field_reconstruction_rejects_underflow_out_of_range_and_wrong_receiver() {
    for (guard, value) in [
        ("countdown.remaining >= 0", "countdown.remaining - 1"),
        ("countdown.remaining > 0", "countdown.remaining + 1"),
        ("other.remaining > 0", "countdown.remaining - 1"),
        ("countdown.remaining > 0", "other.remaining - 1"),
    ] {
        let diagnostics = construction_diagnostics(&format!(
            "data Countdown {{ remaining: u64 [0..=5]; }}
             machine rebuild(countdown: Countdown, other: Countdown) -> Countdown {{
                 transition {guard} {{
                     true -> Countdown {{ remaining: {value} }}
                     false -> Countdown {{ remaining: 0 }}
                 }}
             }}"
        ));
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("field `remaining` cannot be proven within its declared range `0..=5`")),
            "{guard}, {value}: {diagnostics:?}"
        );
    }
}

#[test]
fn field_reconstruction_does_not_import_mutable_or_reference_guard_facts() {
    for parameter in [
        "mut countdown: Countdown",
        "countdown: &Countdown",
        "countdown: &mut Countdown",
    ] {
        let diagnostics = construction_diagnostics(&format!(
            "data Countdown {{ remaining: u64 [0..=5]; }}
             machine rebuild({parameter}) -> Countdown {{
                 transition countdown.remaining > 0 {{
                     true -> Countdown {{ remaining: countdown.remaining - 1 }}
                     false -> Countdown {{ remaining: 0 }}
                 }}
             }}"
        ));
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("field `remaining` cannot be proven within its declared range `0..=5`")),
            "{parameter}: {diagnostics:?}"
        );
    }
}

#[test]
fn retained_case_names_in_value_positions_keep_construction_obligations() {
    for (case, expected) in [("Empty", "omits gated field"), ("Payload", "has a payload")] {
        let mut program = typed(&format!(
            "data Choice [copy] {{ value: u32 [1..=9]; case Empty; case Payload(item: u32); }}
             machine reference(value: &Choice) -> bool {{ value in Choice::{case} }}
             machine make() -> Choice {{ Choice::Empty {{ value: 1 }} }}"
        ));
        let mut diagnostics = Vec::new();
        validate_struct_literal_fields(&program, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "membership is a declaration role: {diagnostics:?}"
        );
        let return_expression = |name: &str| {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap();
            let state = &program.machine_states(machine)[0];
            let [StatementNode::Expression(expression)] =
                program.statement_table.statements(state.statement_nodes)
            else {
                panic!("one result expression");
            };
            *expression
        };
        let reference = return_expression("reference");
        let destination = return_expression("make");
        let ExpressionNode::Binary(membership) = program.expression_table.expression(reference)
        else {
            panic!("case membership");
        };
        let case_name = program
            .expression_table
            .expression(membership.right)
            .clone();
        assert!(matches!(case_name, ExpressionNode::Name(_)));
        *program.expression_table.expression_mut(destination) = case_name;
        validate_struct_literal_fields(&program, &mut diagnostics);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}
