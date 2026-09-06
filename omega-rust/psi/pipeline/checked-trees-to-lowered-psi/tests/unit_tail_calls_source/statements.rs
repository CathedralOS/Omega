//! Standalone Unit expressions perform work without becoming scalar values.

use super::*;
use checked_trees::CheckedScalarExpressionRole;

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

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn standalone_unit_call_before_later_work_checks_without_return_authority() {
    for callee_result in ["", "-> ()"] {
        for (result, continuation) in [("", "let after: u16 = 23;"), ("-> u16", "23u16")] {
            let source = source(
                result,
                callee_result,
                &format!("records[0].record(17); {continuation}"),
            );
            let typed = typed(&source);
            let root = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Root::enter")
                .unwrap();
            let state = &typed.machine_states(root)[0];
            let StatementNode::Expression(expression) =
                typed.statement_table.statements(state.statement_nodes)[0]
            else {
                panic!("the indexed call must exercise source expression-statement admission")
            };
            assert!(validation::unit_statement_call_is_supported(
                &typed, root, state, expression,
            ));
            assert!(
                !validation::unit_return_call_is_supported(&typed, root, state, expression),
                "the public tail-only contract must remain unchanged"
            );
            typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap_or_else(|diagnostics| {
                panic!("standalone Unit work must check: {diagnostics:#?}\n{source}")
            });
        }
    }
}

#[test]
fn standalone_unit_call_retains_scalar_operands_without_a_scalar_return_root() {
    for argument in ["17u16", "identity(identity(17u16))"] {
        let checked = checked(&source(
            "-> u16",
            "",
            &format!("records[0].record({argument}); 23u16"),
        ));
        let root = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .unwrap();
        let state = &checked.machine_states(root)[0];
        let pure = checked
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter()
            .filter(|expression| {
                expression.state == state.symbol && expression.statement_ordinal == 0
            })
            .map(|expression| expression.role);
        let computed = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .filter(|(_, expression)| {
                expression.state == state.symbol && expression.statement_ordinal == 0
            })
            .map(|(_, expression)| expression.role);
        let roles = pure.chain(computed).collect::<Vec<_>>();
        assert!(
            roles.contains(&CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal: 0,
                argument_ordinal: 0,
            }),
            "the exact standalone call operand must retain its source coordinate: {argument}: {roles:?}"
        );
        assert!(
            !roles.contains(&CheckedScalarExpressionRole::Return),
            "Unit work is not the later scalar return"
        );
    }
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
        let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed(&source))
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
fn standalone_unit_call_requires_its_exact_ordinary_target() {
    let typed = typed(&source(
        "",
        "",
        "records[0].record(17); let after: u16 = 23;",
    ));
    let root = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &typed.machine_states(root)[0];
    let StatementNode::Expression(expression) =
        typed.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("indexed Unit expression statement")
    };
    let scalar_target = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "identity")
        .map(|machine| typed.machine_states(machine)[0].symbol)
        .unwrap();
    for modifier in ["missing target", "scalar target", "machine argument"] {
        let mut changed = typed.clone();
        let ExpressionNode::Call(call) = changed.expression_table.expression_mut(expression) else {
            panic!("indexed Unit call")
        };
        match modifier {
            "missing target" => call.target_symbol = symbols::SymbolHandle::invalid(),
            "scalar target" => call.target_symbol = scalar_target,
            "machine argument" => {
                call.machine_arguments =
                    Box::new([typed_trees::expression::StaticMachineArgument {
                        path: Box::new([]),
                        application: None,
                        const_literal: None,
                        evidence_projection: None,
                        symbol: scalar_target,
                    }]);
            }
            _ => unreachable!(),
        }
        assert!(
            !validation::unit_statement_call_is_supported(&changed, root, state, expression),
            "{modifier} cannot acquire standalone Unit authority"
        );
    }
}

#[test]
fn reused_unit_statement_handle_cannot_authorize_a_scalar_return_tail() {
    let mut typed = typed(&source("-> u16", "", "records[0].record(17); 23u16"));
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
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
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
        let mut typed = checked(&source).typed;
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
        let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect_err("a standalone Unit occurrence cannot authorize nested value reuse");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("does not return a value but is used in a VALUE position")),
            "nested reuse in `{continuation}` must retain its value-use rejection: {diagnostics:#?}"
        );
    }
}
