use super::{Lexer, lower_syntax_trees, parse_syntax_trees};
use symbol_resolved_trees::statement::StatementNode;

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
