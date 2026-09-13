use super::*;
use std::cell::Cell;

fn program(body: &str, destination: PrimitiveType) -> (TypedTrees, ExpressionHandle) {
    let source = format!(
        "machine choose() -> {} {{ {body} }}
         machine number(value: u8) -> u8 {{ value }}
         machine truth(value: bool) -> bool {{ value }}
         machine never(value: u8) -> u8 {{ value }}",
        destination.name(),
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    let state = &program.machine_states(&program.machines()[0])[0];
    let [typed_trees::statement::StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        panic!("one source expression");
    };
    let expression = *expression;
    (program, expression)
}

struct Calls<'program> {
    program: &'program TypedTrees,
    executed: Cell<usize>,
}

impl Calls<'_> {
    fn target(&self, target: symbols::SymbolHandle) -> Result<(&Machine, &State), String> {
        let mut selected = self.program.machines().iter().filter_map(|machine| {
            let states = self.program.machine_states(machine);
            let state = if machine.symbol == target {
                states.first()
            } else {
                states.iter().find(|state| state.symbol == target)
            }?;
            Some((machine, state))
        });
        let target = selected.next().ok_or("selected callee")?;
        if selected.next().is_some() {
            return Err("ambiguous callee".into());
        }
        Ok(target)
    }
}

impl ConstantCalls for Calls<'_> {
    fn validate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(PrimitiveType, Vec<Diagnostic>), String> {
        let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
        else {
            return Err("expected call".into());
        };
        let (_, entry) = self.target(call.target_symbol)?;
        let parameters = self.program.state_parameters(entry);
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        if arguments.len() != parameters.len() {
            return Err("call arity".into());
        }
        let caller = &self.program.machines()[0];
        let state = &self.program.machine_states(caller)[0];
        let mut warnings = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            let primitive = self
                .program
                .primitive_type_reference(parameter.type_reference)
                .ok_or("primitive parameter")?;
            warnings.extend(validate_with_calls(
                self.program,
                caller,
                state,
                *argument,
                primitive,
                self,
            )?);
        }
        Ok((
            self.program
                .primitive_type_reference(entry.return_type)
                .ok_or("primitive result")?,
            warnings,
        ))
    }

    fn evaluate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        self.executed.set(self.executed.get() + 1);
        let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
        else {
            return Err("expected call".into());
        };
        let forbidden = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "never")
            .expect("forbidden callee");
        let (callee, entry) = self.target(call.target_symbol)?;
        if callee.symbol == forbidden.symbol {
            return Err("skipped call executed".into());
        }
        let [parameter] = self.program.state_parameters(entry) else {
            return Err("identity call requires one parameter".into());
        };
        let [argument] = self
            .program
            .expression_table
            .expression_handles(call.arguments)
        else {
            return Err("identity call requires one argument".into());
        };
        let caller = &self.program.machines()[0];
        evaluate_with_calls(
            self.program,
            caller,
            &self.program.machine_states(caller)[0],
            *argument,
            self.program
                .primitive_type_reference(parameter.type_reference)
                .ok_or("primitive parameter")?,
            self,
        )
    }
}

#[test]
fn scalar_calls_compose_with_arithmetic_boolean_logic_and_match() {
    for (body, destination, expected, executions) in [
        (
            "number(number(7)) / number(2) * 2",
            PrimitiveType::U8,
            "6",
            3,
        ),
        (
            "truth(true) && truth(false)",
            PrimitiveType::Bool,
            "false",
            2,
        ),
        (
            "false && (never(1u8 / 0) == 0u8)",
            PrimitiveType::Bool,
            "false",
            0,
        ),
        ("true || (never(1) == 0u8)", PrimitiveType::Bool, "true", 0),
        (
            "match truth(true) { true -> number(7), false -> never(0) }",
            PrimitiveType::U8,
            "7",
            2,
        ),
        (
            "(match truth(false) { true -> 7 / 2, false -> 9 / 2 }) * 2",
            PrimitiveType::U8,
            "9",
            1,
        ),
    ] {
        let (program, expression) = program(body, destination);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let calls = Calls {
            program: &program,
            executed: Cell::new(0),
        };
        validate_with_calls(&program, machine, state, expression, destination, &calls)
            .expect("static call shapes");
        assert_eq!(calls.executed.get(), 0, "static pass does not invoke calls");
        let (value, _) =
            evaluate_with_calls(&program, machine, state, expression, destination, &calls)
                .unwrap_or_else(|error| panic!("{body}: {error}"));
        assert_eq!(value.display, expected, "{body}");
        assert_eq!(calls.executed.get(), executions, "{body}");
        assert!(
            evaluate(&program, machine, state, expression, destination).is_err(),
            "call-free API cannot acquire invocation authority"
        );
    }
}

#[test]
fn skipped_calls_still_validate_argument_carriers_and_anonymous_landings() {
    for body in [
        "false && (never(1u64) == 0u8)",
        "false && (never(256) == 0u8)",
        "false && (never(1 / 2) == 0u8)",
        "false && (never(1 / 0) == 0u8)",
    ] {
        let (program, expression) = program(body, PrimitiveType::Bool);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let calls = Calls {
            program: &program,
            executed: Cell::new(0),
        };
        assert!(
            evaluate_with_calls(
                &program,
                machine,
                state,
                expression,
                PrimitiveType::Bool,
                &calls
            )
            .is_err(),
            "{body}"
        );
        assert_eq!(calls.executed.get(), 0, "{body}");
    }
}

#[test]
fn call_graph_rejects_stale_argument_spans_and_cycles_before_callbacks() {
    let (original, root) = program("number(7) + 0u8", PrimitiveType::U8);
    let ExpressionNode::Binary(binary) = original.expression_table.expression(root) else {
        panic!("source arithmetic");
    };
    let expression = binary.left;
    let ExpressionNode::Call(call) = original.expression_table.expression(expression) else {
        panic!("call");
    };
    let arguments = call.arguments;
    for replacement in [expression, ExpressionHandle::invalid()] {
        let mut program = original.clone();
        program
            .expression_table
            .set_expression_handle_at_offset(arguments, 0, replacement);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let calls = Calls {
            program: &program,
            executed: Cell::new(0),
        };
        let error = evaluate_with_calls(
            &program,
            machine,
            state,
            expression,
            PrimitiveType::U8,
            &calls,
        )
        .expect_err("malformed argument graph");
        assert!(error.contains("invalid or cyclic"), "{error}");
    }
    let mut program = original.clone();
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
        panic!("call");
    };
    call.arguments = arena::HandleSpan::from_parts(
        arena::Handle::from_parts(
            arguments.start().arena_index(),
            arguments.start().generation() + 1,
        ),
        arguments.count(),
    );
    let machine = &program.machines()[0];
    let calls = Calls {
        program: &program,
        executed: Cell::new(0),
    };
    let error = validate_with_calls(
        &program,
        machine,
        &program.machine_states(machine)[0],
        expression,
        PrimitiveType::U8,
        &calls,
    )
    .expect_err("stale argument span");
    assert!(error.contains("argument span"), "{error}");
}

#[test]
fn call_result_cannot_change_its_validated_carrier_or_encoding() {
    let identity = CanonicalConstIdentity::integer("u64", 7);
    let value = CanonicalConstValue::new(identity.type_name, identity.encoding, "7");
    assert!(call_value(&value, Shape::Integer(LandedIntegerType::U8)).is_err());
    let identity = CanonicalConstIdentity::integer("u8", 256);
    let value = CanonicalConstValue::new(identity.type_name, identity.encoding, "256");
    assert!(call_value(&value, Shape::Integer(LandedIntegerType::U8)).is_err());
    assert!(
        call_value(
            &CanonicalConstValue::boolean(true),
            Shape::Integer(LandedIntegerType::U8)
        )
        .is_err()
    );
}
