use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionNode;
use psi_validation::validate_program;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

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
        let diagnostics = validate_program(&typed(&source)).expect_err("capture must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&format!("uses `{name}`"))),
            "{diagnostics:?}\n{source}"
        );
    }
}

#[test]
fn executable_states_accept_explicit_forwarding_and_prior_locals() {
    for source in [
        "machine run(input: u64) { transition { _ -> next(input) } state next(current: u64) { let copy: u64 = current; let again: u64 = copy; } }",
        "machine store(value: &mut u64) { value = 1; } machine run(input: &mut u64) { transition { _ -> next(input) } state next(current: &mut u64) { store(current); } }",
        "data Packet { value: u64; } machine Packet::run(&mut self) { transition { _ -> next() } state next(&mut self) { self.value = 1; } }",
    ] {
        let result = validate_program(&typed(source));
        assert!(result.is_ok(), "{result:?}\n{source}");
    }
}

#[test]
fn same_spelling_foreign_parameter_identity_cannot_grant_read_or_write_access() {
    for expression in ["let copy: u64 = value;", "store(value);", "value = 1;"] {
        let source = format!(
            "machine store(value: &mut u64) {{ value = 1; }} machine run(value: &mut u64) {{ transition {{ _ -> next(value) }} state next(value: &mut u64) {{ {expression} }} }}"
        );
        let mut program = typed(&source);
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
    assert!(validate_program(&typed(source)).is_err());
}

#[test]
fn local_scope_begins_after_its_initializer() {
    for body in [
        "let value: u64 = value;",
        "let copy: u64 = value; let value: u64 = 7;",
    ] {
        let program = typed(&format!("machine run() {{ {body} }}"));
        let diagnostics = validate_program(&program).expect_err("later local is not live");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uses `value`")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn projected_statement_receivers_retain_the_exact_current_state_root() {
    for body in [
        "packet.inner.touch();",
        "let local: Packet; local.inner.touch();",
    ] {
        let source = format!(
            "data Inner {{ value: u64; }} data Packet {{ inner: Inner; }} machine Inner::touch(&mut self) {{ self.value = 1; }} machine run(packet: &mut Packet) {{ {body} }}"
        );
        let program = typed(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "run")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let call = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                if let psi_typed_trees::statement::StatementNode::Call(call) = statement {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("projected statement call");
        validate_program(&program).unwrap_or_else(|diagnostics| {
            panic!(
                "{diagnostics:?}: {call:?}; root {:?}",
                program.symbols.get(call.receiver_root_symbol)
            )
        });
        assert_ne!(call.receiver_root_symbol, call.receiver_symbol);
        assert_eq!(
            program.symbols.get(call.receiver_root_symbol).parent,
            state.symbol
        );
        assert_eq!(
            program
                .statement_table
                .name_path_members(call.receiver)
                .last()
                .unwrap()
                .as_str(),
            "inner"
        );
    }
}

#[test]
fn projected_statement_receivers_reject_missing_or_foreign_state_roots() {
    let source = "data Inner { value: u64; } data Packet { inner: Inner; } machine Inner::touch(&mut self) { self.value = 1; } machine run(packet: &mut Packet) { transition { _ -> next(packet) } state next(packet: &mut Packet) { packet.inner.touch(); } }";
    let program = typed(source);
    validate_program(&program).expect("explicit forwarded receiver");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let states = program.machine_states(machine);
    let foreign = program.state_parameters(&states[0])[0].symbol;
    let current = states
        .iter()
        .find(|state| state.name.as_str() == "next")
        .unwrap();
    let statements = current.statement_nodes;
    for root in [psi_symbols::SymbolHandle::invalid(), foreign] {
        let mut altered = program.clone();
        let call = altered
            .statement_table
            .statements_mut(statements)
            .iter_mut()
            .find_map(|statement| {
                if let psi_typed_trees::statement::StatementNode::Call(call) = statement {
                    Some(call)
                } else {
                    None
                }
            })
            .unwrap();
        call.receiver_root_symbol = root;
        let diagnostics = validate_program(&altered)
            .expect_err("absent or sibling storage cannot authorize receiver");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uses `packet`")),
            "{diagnostics:?}"
        );
    }
    let capture = source
        .replace("next(packet)", "next()")
        .replace("state next(packet: &mut Packet)", "state next()");
    assert!(
        validate_program(&typed(&capture)).is_err(),
        "nested fields do not permit implicit entry capture"
    );
}
