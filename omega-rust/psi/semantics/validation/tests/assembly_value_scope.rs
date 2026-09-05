use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use validation::validate_program;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn assert_scope_rejected(program: &TypedTrees) {
    let diagnostics =
        validate_program(program).expect_err("assembly assertion capture must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("asm assertion value")),
        "{diagnostics:?}"
    );
}

#[test]
fn assembly_assertions_reject_entry_parameters_and_locals() {
    for kind in ["requires", "ensures"] {
        for (parameters, setup, expression) in [
            ("hidden: u64", "", "hidden == hidden"),
            ("", "let hidden: u64 = 7;", "hidden == hidden"),
            ("hidden: Packet", "", "hidden.value == hidden.value"),
            ("hidden: [u64; 2]", "", "hidden[0] == hidden[0]"),
        ] {
            let source = format!(
                "data Packet {{ value: u64; }} machine run({parameters}) {{ {setup} transition {{ _ -> next() }} state next() {{ asm where {kind} ({expression}) {{ lfence }} }} }}"
            );
            assert_scope_rejected(&typed(&source));
        }
    }
}

#[test]
fn assembly_assertions_accept_current_parameters_prior_locals_and_self() {
    for kind in ["requires", "ensures"] {
        for source in [
            format!(
                "machine run(input: u64) {{ transition {{ _ -> next(input) }} state next(current: u64) {{ asm where {kind} (current == current) {{ lfence }} }} }}"
            ),
            format!(
                "machine run() {{ let prior: u64 = 7; asm where {kind} (prior == prior) {{ lfence }} }}"
            ),
            format!(
                "data Owner {{ value: u64; }} machine Owner::run(&self) {{ transition {{ _ -> next() }} state next(&self) {{ asm where {kind} (self.value == self.value) {{ lfence }} }} }}"
            ),
        ] {
            let result = validate_program(&typed(&source));
            assert!(result.is_ok(), "{result:?}\n{source}");
        }
    }
}

#[test]
fn assembly_assertions_reject_absent_self_and_later_local() {
    for kind in ["requires", "ensures"] {
        for source in [
            format!(
                "data Owner {{ value: u64; }} machine Owner::run(&self) {{ transition {{ _ -> next() }} state next() {{ asm where {kind} (self.value == self.value) {{ lfence }} }} }}"
            ),
            format!(
                "machine run() {{ asm where {kind} (later == later) {{ lfence }} let later: u64 = 7; }}"
            ),
        ] {
            assert_scope_rejected(&typed(&source));
        }
    }
}

#[test]
fn assembly_assertions_reject_same_spelling_foreign_parameter_identity() {
    let program = typed(
        "machine run(value: u64) { transition { _ -> next(value) } state next(value: u64) { asm where requires (value == value) ensures (value == value) { lfence } } }",
    );
    validate_program(&program).expect("current-state operands are valid");
    let machine = &program.machines()[0];
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
            matches!(expression, ExpressionNode::Name(path) if path.symbol == own).then_some(handle)
        })
        .collect();
    assert!(!handles.is_empty());
    for (head, selected) in [(foreign, foreign), (own, foreign), (foreign, own)] {
        let mut altered = program.clone();
        for handle in &handles {
            let ExpressionNode::Name(path) = altered.expression_table.expression_mut(*handle)
            else {
                unreachable!()
            };
            path.head_symbol = head;
            path.symbol = selected;
        }
        assert_scope_rejected(&altered);
    }
}
