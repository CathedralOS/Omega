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
