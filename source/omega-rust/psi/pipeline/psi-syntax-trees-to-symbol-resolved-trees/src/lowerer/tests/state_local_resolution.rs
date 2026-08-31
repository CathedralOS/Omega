use super::{Lexer, lower_syntax_trees, parse_syntax_trees};

#[test]
fn state_local_receiver_wins_over_same_named_enclosing_state() {
    let source = r#"
        data Plan {}
        machine Plan::with(&self) -> Plan {
            transition { _ -> (Plan {}) }
        }

        data Owner {}
        machine Owner::plan() -> Plan {
            let plan: Plan = Plan {};
            transition { _ -> (plan.with()) }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize local call");
    let syntax = parse_syntax_trees(&tokens).expect("parse local call");
    let program = lower_syntax_trees(&syntax).expect("resolve local call");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::plan")
        .expect("Owner::plan machine");
    let state = program
        .machine_state_handles(machine.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("Owner::plan state");
    let statements = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes);
    let [
        psi_symbol_resolved_trees::statement::StatementNode::LocalData(local),
        psi_symbol_resolved_trees::statement::StatementNode::Transition(transition),
    ] = statements
    else {
        panic!("local declaration followed by transition")
    };
    let psi_symbol_resolved_trees::statement::TransitionTargetNode::Value(value) = program
        .tables
        .bodies
        .statements
        .transition_target(transition.target)
    else {
        panic!("value transition")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*value)
    else {
        panic!("local receiver call")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Name(receiver) =
        program.tables.bodies.expressions.expression(call.receiver)
    else {
        panic!("named local receiver")
    };

    assert_eq!(receiver.head_symbol, local.symbol);
    assert_eq!(receiver.symbol, local.symbol);
    assert_ne!(receiver.symbol, state.symbol);
}
