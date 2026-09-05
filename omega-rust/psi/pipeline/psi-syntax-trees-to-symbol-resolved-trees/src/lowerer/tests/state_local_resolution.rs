use super::{Lexer, lower_syntax_trees, parse_syntax_trees};

#[test]
fn computed_receiver_does_not_select_a_same_spelling_free_machine() {
    for receiver in ["produce()", "bucket().cell", "array()[0]"] {
        let source = format!(
            r#"
            data Cell {{}} data Bucket {{ cell: Cell; }}
            machine Cell::read(&self) -> u64 {{ 1 }}
            machine read() -> u64 {{ 2 }}
            machine produce() -> Cell {{ Cell {{}} }}
            machine bucket() -> Bucket {{ Bucket {{ cell: Cell {{}} }} }}
            machine array() -> [Cell; 1] {{ [Cell {{}}] }}
            machine run() {{ let result: u64 = {receiver}.read(); }}
        "#
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let program = lower_syntax_trees(&syntax).expect("resolve");
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "run")
            .expect("caller");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let psi_symbol_resolved_trees::statement::StatementNode::LocalData(local) = &program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)[0]
        else {
            panic!("result");
        };
        let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression(local.initial_value)
        else {
            panic!("method");
        };
        assert!(
            !call.target_symbol.is_valid(),
            "{receiver}: declared result typing must select the method"
        );
    }
}

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
    let callee = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Plan::with")
        .expect("attached callee");
    let callee_state = program.machine_state(program.machine_state_handles(callee.states)[0]);
    assert_eq!(call.target_symbol, callee_state.symbol);
}

#[test]
fn local_call_targets_follow_prior_declarations_not_receiver_spelling() {
    use psi_symbol_resolved_trees::expression::ExpressionNode;
    use psi_symbol_resolved_trees::statement::StatementNode;
    use psi_symbols::SymbolHandle;

    let cases = [
        (
            "plain",
            "let local: Other = Other {}; local.read(); let answer: u64 = local.read();",
            "Other::read",
            2,
        ),
        (
            "field_shadow",
            "let value: Other = Other {}; value.read(); let answer: u64 = value.read();",
            "Other::read",
            2,
        ),
        (
            "type_shadow",
            "let Pair: Other = Other {}; Pair.read(); let answer: u64 = Pair.read();",
            "Other::read",
            2,
        ),
        (
            "shared_reference",
            "let local: &Other = &self.other; local.read(); let answer: u64 = local.read();",
            "Other::read",
            2,
        ),
        (
            "exclusive_reference",
            "let local: &mut Other = &mut self.other; local.read(); let answer: u64 = local.read();",
            "Other::read",
            2,
        ),
        (
            "missing_method",
            "let local: Other = Other {}; local.missing(); let answer: u64 = local.missing();",
            "",
            2,
        ),
        (
            "missing_shadow_method",
            "let Pair: Other = Other {}; Pair.missing(); let answer: u64 = Pair.missing();",
            "",
            2,
        ),
        (
            "later_local",
            "local.read(); let answer: u64 = local.read(); let local: Other = Other {};",
            "",
            2,
        ),
        ("self_initializer", "let Pair: Other = Pair.read();", "", 1),
        (
            "whole_array",
            "let local: [Other; 1] = [Other {}]; local.read(); let answer: u64 = local.read();",
            "",
            2,
        ),
        (
            "whole_slice",
            "let local: &[Other] = &self.others; local.read(); let answer: u64 = local.read();",
            "",
            2,
        ),
    ];
    for (name, body, expected, count) in cases {
        let source = format!(
            "data Pair {{}} data Other {{}}
            data Owner {{ value: Pair; other: Other; others: [Other; 1]; }}
            machine Pair::read(&self) -> u64 {{ 1 }}
            machine Pair::missing(&self) -> u64 {{ 2 }}
            machine Other::read(&self) -> u64 {{ 3 }}
            machine missing() -> u64 {{ 4 }}
            machine Owner::run(&mut self) {{ {body} }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let program = lower_syntax_trees(&syntax).expect("resolve");
        let expected = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == expected)
            .map(|machine| {
                program
                    .machine_state(program.machine_state_handles(machine.states)[0])
                    .symbol
            })
            .unwrap_or_else(SymbolHandle::invalid);
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "Owner::run")
            .expect("caller");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let targets = program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::Call(call) => Some(call.target_symbol),
                StatementNode::LocalData(local) => match program
                    .tables
                    .bodies
                    .expressions
                    .expression(local.initial_value)
                {
                    ExpressionNode::Call(call) => Some(call.target_symbol),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            targets.len(),
            count,
            "{name}: both statement and value positions must be checked"
        );
        assert!(
            targets.iter().all(|target| *target == expected),
            "{name}: {targets:?} must select {expected:?}"
        );
    }
}
