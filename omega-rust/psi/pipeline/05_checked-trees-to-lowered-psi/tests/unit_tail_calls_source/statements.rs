//! Standalone Unit expressions perform work without becoming scalar values.
use super::StatementNode;

fn source(result: &str, callee_result: &str, body: &str) -> String {
    format!(
        "data Record [copy] {{ value: u16; }}
         machine Record::record(&write self, value: u16) {callee_result} {{ self.value = value; }}
         machine identity(value: u16) -> u16 {{ value }}
         machine consume(value: u16) {{}}
         data Root {{}}
         machine Root::enter(records: &write [Record; 2]) {result} {{ {body} }}"
    )
}

#[test]
fn standalone_unit_admission_does_not_grant_value_or_scalar_tail_use() {
    for (result, body) in [
        ("", "let invalid: u16 = records[0].record(17);"),
        ("", "let invalid: () = records[0].record(17);"),
        ("", "consume(records[0].record(17));"),
        ("", "let invalid: u16 = records[0].record(17) + 1u16;"),
        ("-> u16", "records[0].record(17)"),
    ] {
        let source = source(result, "-> ()", body);
        let diagnostics = crate::front_end::checked_program_result(&source)
            .expect_err("Unit cannot supply a local, argument, scalar operand, or scalar tail");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("does not return a value but is used in a VALUE position")
            }),
            "the Unit use must be rejected explicitly: {diagnostics:#?}\n{source}"
        );
    }
}

#[test]
fn reused_unit_statement_handle_cannot_authorize_a_scalar_return_tail() {
    let mut typed =
        crate::front_end::typed_program(&source("-> u16", "", "records[0].record(17); 23u16"));
    let root = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap()
        .clone();
    let state = typed.machine_states(&root)[0].clone();
    let statements = typed.statement_table.statements_mut(state.statement_nodes);
    let StatementNode::Expression(expression) = statements[0] else {
        panic!("standalone Unit call")
    };
    *statements.last_mut().unwrap() = StatementNode::Expression(expression);
    assert!(
        !validation::unit_statement_call_is_supported(&typed, &root, &state, expression),
        "a reused direct root has no unique statement occurrence"
    );
    assert!(
        !validation::unit_return_call_is_supported(&typed, &root, &state, expression),
        "the scalar return contract must still reject the Unit tail"
    );
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect_err("an earlier statement cannot authorize the reused Unit tail");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not return a value but is used in a VALUE position")),
        "{diagnostics:#?}"
    );
}

#[test]
fn reused_unit_statement_handle_cannot_authorize_a_nested_value_use() {
    for continuation in ["let invalid: u16 = 23;", "consume(23);"] {
        let source = source(
            "",
            "-> ()",
            &format!("records[0].record(17); {continuation}"),
        );
        let mut typed = crate::front_end::checked_program(&source).typed;
        let root = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .unwrap();
        let state = typed.machine_states(root)[0].clone();
        let statements = typed.statement_table.statements(state.statement_nodes);
        let StatementNode::Expression(expression) = statements[0] else {
            panic!("standalone Unit call")
        };
        match statements[1].clone() {
            StatementNode::LocalData(_) => {
                let StatementNode::LocalData(local) =
                    &mut typed.statement_table.statements_mut(state.statement_nodes)[1]
                else {
                    unreachable!()
                };
                local.initial_value = expression;
            }
            StatementNode::Call(call) => {
                typed
                    .statement_table
                    .set_expression_handle_at_offset(call.arguments, 0, expression)
            }
            _ => panic!("the second statement must use a scalar initializer or argument"),
        }
        assert_eq!(
            typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter(|statement| matches!(statement, StatementNode::Expression(root) if *root == expression))
                .count(),
            1,
            "nested reuse must not become a second direct statement root"
        );
        let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .expect_err("a standalone Unit occurrence cannot authorize nested value reuse");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("does not return a value but is used in a VALUE position")),
            "nested reuse in `{continuation}` must retain its value-use rejection: {diagnostics:#?}"
        );
    }
}
