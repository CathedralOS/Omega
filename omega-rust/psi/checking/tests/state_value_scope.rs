use typed_trees::expression::ExpressionNode;
use validation::validate_program;

#[test]
fn executable_states_reject_implicit_entry_storage() {
    for (parameters, setup, body, name) in [
        ("input: u64", "", "let copy: u64 = input;", "input"),
        ("", "let saved: u64 = 7;", "let copy: u64 = saved;", "saved"),
        ("value: &mut u64", "", "value = 1;", "value"),
        ("packet: &mut Packet", "", "packet.value = 1;", "packet"),
        ("packet: &mut Packet", "", "packet.touch();", "packet"),
        (
            "packet: &mut Packet",
            "",
            "let copy: u64 = packet.read();",
            "packet",
        ),
    ] {
        let source = format!(
            "data Packet {{ value: u64; }} machine Packet::touch(&mut self) {{ self.value = 1; }} machine Packet::read(&self) -> u64 {{ self.value }} machine run({parameters}) {{ {setup} transition {{ _ -> next() }} state next() {{ {body} }} }}"
        );
        let diagnostics = validate_program(&crate::front_end::typed_program(&source))
            .expect_err("capture must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&format!("uses `{name}`"))),
            "{diagnostics:?}\n{source}"
        );
    }
}

#[test]
fn same_spelling_foreign_parameter_identity_cannot_grant_read_or_write_access() {
    for expression in ["let copy: u64 = value;", "store(value);", "value = 1;"] {
        let source = format!(
            "machine store(value: &mut u64) {{ value = 1; }} machine run(value: &mut u64) {{ transition {{ _ -> next(value) }} state next(value: &mut u64) {{ {expression} }} }}"
        );
        let mut program = crate::front_end::typed_program(&source);
        validate_program(&program).expect("unaltered explicit bindings are valid");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "run")
            .unwrap();
        let states = program.machine_states(machine);
        let current = states
            .iter()
            .find(|state| state.name.as_str() == "next")
            .unwrap();
        let own = program.state_parameters(current)[0].symbol;
        let foreign = program.state_parameters(
            states
                .iter()
                .find(|state| state.symbol != current.symbol)
                .unwrap(),
        )[0]
        .symbol;
        let handles: Vec<_> = program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Name(path) if path.symbol == own)
                    .then_some(handle)
            })
            .collect();
        assert!(!handles.is_empty());
        for handle in handles {
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(handle) else {
                unreachable!()
            };
            path.symbol = foreign;
            path.head_symbol = foreign;
        }
        let diagnostics =
            validate_program(&program).expect_err("foreign same-name parameter must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uses `value`")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn mutable_sibling_parameter_cannot_authorize_shared_current_parameter() {
    let source = "machine store(value: &mut u64) { value = 1; } machine run(value: &mut u64) { transition { _ -> next(value) } state next(value: &u64) { store(value); } }";
    assert!(validate_program(&crate::front_end::typed_program(source)).is_err());
}
