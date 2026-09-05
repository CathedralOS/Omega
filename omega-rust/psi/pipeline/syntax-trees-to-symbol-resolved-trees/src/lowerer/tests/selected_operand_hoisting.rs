use super::{Lexer, lower_syntax_trees, parse_syntax_trees};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::statement::{StatementNode, TransitionTargetNode};

fn resolved(source: &str) -> SymbolResolvedTrees {
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
    lower_syntax_trees(&syntax).unwrap()
}

fn value_statements(program: &SymbolResolvedTrees) -> &[StatementNode] {
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_state(program.machine_state_handles(machine.states)[0]);
    program
        .tables
        .bodies
        .statements
        .statements(entry.statement_nodes)
}

fn return_expressions(program: &SymbolResolvedTrees) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    for statement in value_statements(program) {
        match statement {
            StatementNode::Expression(expression) => expressions.push(*expression),
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
                    let TransitionTargetNode::Value(expression) =
                        program.tables.bodies.statements.transition_target(target)
                    else {
                        panic!("authored value return remains a value return");
                    };
                    expressions.push(*expression);
                }
            }
            _ => panic!("computed returns must not create enclosing bindings"),
        }
    }
    expressions
}

#[test]
fn selected_arm_call_casts_do_not_create_enclosing_bindings() {
    let source = r#"
        machine read() -> u16 { 7u16 }
        machine value(selected: bool, flag: bool) -> bool {
            transition selected {
                true -> finish(flag && ((read() as u8) == 7u8))
                false -> finish(flag || ((read() as u8) == 7u8))
            }
            state finish(result: bool) -> bool { result }
        }
    "#;
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
    let program = lower_syntax_trees(&syntax).unwrap();
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let statements = program
        .tables
        .bodies
        .statements
        .statements(entry.statement_nodes);
    assert_eq!(statements.len(), 2);
    assert!(
        statements
            .iter()
            .all(|statement| matches!(statement, StatementNode::Transition(_)))
    );
}

#[test]
fn selective_rhs_call_casts_stay_inside_the_authored_initializer() {
    for connective in ["&&", "||"] {
        let source = format!(
            r#"
            machine read() -> u16 {{ 7u16 }}
            machine value(flag: bool) -> bool {{
                let answer: bool = flag {connective} ((read() as u8) == 7u8);
                answer
            }}
        "#
        );
        let syntax = parse_syntax_trees(&Lexer::new(&source).tokenize().unwrap()).unwrap();
        let program = lower_syntax_trees(&syntax).unwrap();
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        let entry = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let statements = program
            .tables
            .bodies
            .statements
            .statements(entry.statement_nodes);
        assert_eq!(statements.len(), 2, "{connective}: {statements:?}");
        let StatementNode::LocalData(local) = &statements[0] else {
            panic!("authored initializer");
        };
        assert_eq!(local.name.as_str(), "answer");
    }
}

#[test]
fn return_call_casts_retain_authored_left_to_right_operands() {
    for body in [
        "first() + (second() as u16)",
        "transition { _ -> (first() + (second() as u16)) }",
        "transition selected {
            true -> (first() + (second() as u16))
            false -> (first() + (second() as u16))
        }",
    ] {
        let program = resolved(&format!(
            "machine first() -> u16 {{ 1u16 }}
             machine second() -> u8 {{ 2u8 }}
             machine value(selected: bool) -> u16 {{ {body} }}"
        ));
        let expressions = &program.tables.bodies.expressions;
        let returns = return_expressions(&program);
        assert_eq!(returns.len(), if body.contains("true ->") { 2 } else { 1 });
        for result in returns {
            let ExpressionNode::Binary(binary) = expressions.expression(result) else {
                panic!("authored addition");
            };
            let ExpressionNode::Call(first) = expressions.expression(binary.left) else {
                panic!("earlier call retains its left operand position");
            };
            let ExpressionNode::Cast(cast) = expressions.expression(binary.right) else {
                panic!("later call retains its surrounding cast");
            };
            let ExpressionNode::Call(second) = expressions.expression(cast.value) else {
                panic!("later call must not execute through an enclosing binding");
            };
            assert_eq!(first.target.as_str(), "first");
            assert_eq!(second.target.as_str(), "second");
            assert!(first.target_symbol.is_valid());
            assert!(second.target_symbol.is_valid());
            assert_ne!(first.target_symbol, second.target_symbol);
        }
    }
}

#[test]
fn return_cast_calls_stay_nested_through_outer_arithmetic() {
    for body in [
        "(identity(input) as u16) + 1u16",
        "transition { _ -> ((identity(input) as u16) + 1u16) }",
    ] {
        let program = resolved(&format!(
            "machine identity(input: u8) -> u8 {{ input }}
             machine value(input: u8) -> u16 {{ {body} }}"
        ));
        let returns = return_expressions(&program);
        assert_eq!(returns.len(), 1);
        let expressions = &program.tables.bodies.expressions;
        let ExpressionNode::Binary(binary) = expressions.expression(returns[0]) else {
            panic!("returned arithmetic");
        };
        let ExpressionNode::Cast(cast) = expressions.expression(binary.left) else {
            panic!("call-result cast");
        };
        let ExpressionNode::Call(call) = expressions.expression(cast.value) else {
            panic!("call retains its evaluation position");
        };
        assert_eq!(call.target.as_str(), "identity");
        assert_eq!(expressions.expression_handles(call.arguments).len(), 1);
    }
}

#[test]
fn initializer_call_casts_retain_authored_left_to_right_operands() {
    for declaration in ["let answer: u16", "let mut answer: u16"] {
        let program = resolved(&format!(
            "machine first() -> u16 {{ 1u16 }}
             machine second() -> u8 {{ 2u8 }}
             machine value() -> u16 {{
                 {declaration} = first() + (second() as u16);
                 answer
             }}"
        ));
        let statements = value_statements(&program);
        assert_eq!(statements.len(), 2, "{declaration}: {statements:?}");
        let StatementNode::LocalData(local) = &statements[0] else {
            panic!("authored initializer");
        };
        assert_eq!(local.name.as_str(), "answer");
        assert_eq!(local.is_mutable, declaration.contains("mut"));
        let expressions = &program.tables.bodies.expressions;
        let ExpressionNode::Binary(binary) = expressions.expression(local.initial_value) else {
            panic!("authored addition");
        };
        let ExpressionNode::Call(first) = expressions.expression(binary.left) else {
            panic!("earlier call retains its left operand position");
        };
        let ExpressionNode::Cast(cast) = expressions.expression(binary.right) else {
            panic!("later call retains its surrounding cast");
        };
        let ExpressionNode::Call(second) = expressions.expression(cast.value) else {
            panic!("later call must not execute through an enclosing binding");
        };
        assert_eq!(first.target.as_str(), "first");
        assert_eq!(second.target.as_str(), "second");
        assert!(first.target_symbol.is_valid());
        assert!(second.target_symbol.is_valid());
        assert_ne!(first.target_symbol, second.target_symbol);
    }
}

#[test]
fn initializer_cast_calls_stay_nested_through_outer_arithmetic() {
    let program = resolved(
        "machine read() -> u8 { 7u8 }
         machine value() -> u16 {
             let answer: u16 = (read() as u16) + 1u16;
             answer
         }",
    );
    let statements = value_statements(&program);
    assert_eq!(statements.len(), 2);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("authored initializer");
    };
    assert_eq!(local.name.as_str(), "answer");
    let expressions = &program.tables.bodies.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(local.initial_value) else {
        panic!("initialized arithmetic");
    };
    let ExpressionNode::Cast(cast) = expressions.expression(binary.left) else {
        panic!("call-result cast");
    };
    let ExpressionNode::Call(call) = expressions.expression(cast.value) else {
        panic!("call retains its evaluation position");
    };
    assert_eq!(call.target.as_str(), "read");
}

#[test]
fn assignment_call_casts_retain_authored_left_to_right_operands() {
    let program = resolved(
        "machine first() -> u16 { 1u16 }
         machine second() -> u8 { 2u8 }
         machine value() -> u16 {
             let mut answer: u16 = 0u16;
             answer = first() + (second() as u16);
             answer
         }",
    );
    let statements = value_statements(&program);
    assert_eq!(statements.len(), 3);
    let StatementNode::Assignment(assignment) = &statements[1] else {
        panic!("authored assignment");
    };
    let expressions = &program.tables.bodies.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(assignment.value) else {
        panic!("authored addition");
    };
    let ExpressionNode::Call(first) = expressions.expression(binary.left) else {
        panic!("earlier call retains its left operand position");
    };
    let ExpressionNode::Cast(cast) = expressions.expression(binary.right) else {
        panic!("later call retains its surrounding cast");
    };
    let ExpressionNode::Call(second) = expressions.expression(cast.value) else {
        panic!("later call must not execute through an enclosing binding");
    };
    assert_eq!(first.target.as_str(), "first");
    assert_eq!(second.target.as_str(), "second");
    assert!(first.target_symbol.is_valid());
    assert!(second.target_symbol.is_valid());
    assert_ne!(first.target_symbol, second.target_symbol);
}

#[test]
fn assignment_cast_calls_remain_at_their_authored_roots() {
    for value in ["(read() as u16) + 1u16", "read() as u16"] {
        let program = resolved(&format!(
            "machine read() -> u8 {{ 7u8 }}
             machine value() -> u16 {{
                 let mut answer: u16 = 0u16;
                 answer = {value};
                 answer
             }}"
        ));
        let statements = value_statements(&program);
        assert_eq!(statements.len(), 3);
        let StatementNode::Assignment(assignment) = &statements[1] else {
            panic!("authored assignment");
        };
        let expressions = &program.tables.bodies.expressions;
        let cast_expression = match expressions.expression(assignment.value) {
            ExpressionNode::Binary(binary) => binary.left,
            ExpressionNode::Cast(_) => assignment.value,
            _ => panic!("authored assignment expression"),
        };
        let ExpressionNode::Cast(cast) = expressions.expression(cast_expression) else {
            panic!("authored cast");
        };
        let ExpressionNode::Call(call) = expressions.expression(cast.value) else {
            panic!("call retains its evaluation position");
        };
        assert_eq!(call.target.as_str(), "read");
    }
}

#[test]
fn assignment_operand_indexed_read_hoisting_is_unchanged() {
    let program = resolved(
        "machine value(items: [u8; 4], index: u64) -> u16 {
             let mut answer: u16 = 0u16;
             answer = (items[index] as u16) + 1u16;
             answer
         }",
    );
    let statements = value_statements(&program);
    assert_eq!(statements.len(), 4);
    let StatementNode::LocalData(local) = &statements[1] else {
        panic!("existing indexed-read binding");
    };
    assert!(matches!(
        program
            .tables
            .bodies
            .expressions
            .expression(local.initial_value),
        ExpressionNode::Indexed(_)
    ));
    assert!(matches!(statements[2], StatementNode::Assignment(_)));
}

#[test]
fn return_operand_indexed_read_hoisting_is_unchanged() {
    for body in [
        "(items[index] as u16) + 1u16",
        "transition { _ -> ((items[index] as u16) + 1u16) }",
    ] {
        let program = resolved(&format!(
            "machine value(items: [u8; 4], index: u64) -> u16 {{ {body} }}"
        ));
        let statements = value_statements(&program);
        assert_eq!(statements.len(), 2);
        let StatementNode::LocalData(local) = &statements[0] else {
            panic!("existing indexed-read binding");
        };
        assert!(matches!(
            program
                .tables
                .bodies
                .expressions
                .expression(local.initial_value),
            ExpressionNode::Indexed(_)
        ));
    }
}
