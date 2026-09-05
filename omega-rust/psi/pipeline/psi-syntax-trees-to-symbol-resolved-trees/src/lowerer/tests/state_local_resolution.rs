use super::{Lexer, lower_syntax_trees, parse_syntax_trees};

#[test]
fn contract_membership_values_use_exact_callable_parameters() {
    use psi_symbol_resolved_trees::{domain::ProofFact, expression::ExpressionNode};

    let source = r#"
        domain u64::Small requires self < 10;
        machine run(value: u64)
        requires value in Small
        ensures value in Small
        {
            transition { _ -> next(value) }
            state next(value: u64) requires value in Small {}
        }
    "#;
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize"))
        .expect("parse membership contracts");
    let program = lower_syntax_trees(&syntax).expect("resolve membership contracts");
    let machine = &program.machines[0];
    let states = program.machine_state_handles(machine.states);
    let entry = program.machine_state(states[0]);
    let target = program.machine_state(states[1]);
    let entry_parameter = program.state_parameters(entry.parameters)[0].symbol;
    let target_parameter = program.state_parameters(target.parameters)[0].symbol;
    assert_ne!(entry_parameter, target_parameter);
    for (contracts, expected) in [
        (machine.contracts, entry_parameter),
        (target.contracts, target_parameter),
    ] {
        for contract in program
            .tables
            .declarations
            .signature_contracts
            .span_or_empty(contracts)
        {
            let [ProofFact::Membership(membership)] = program.proof_facts(contract.facts) else {
                panic!("one membership fact");
            };
            let ExpressionNode::Name(path) = program
                .tables
                .bodies
                .expressions
                .expression(membership.value)
            else {
                panic!("parameter membership value");
            };
            assert_eq!(path.symbol, expected);
            assert_eq!(path.head_symbol, expected);
            assert_eq!(
                membership.domain_symbol,
                program.domain_definitions[0].symbol
            );
        }
    }
}

#[test]
fn state_contract_value_arguments_share_the_explicit_frontier() {
    use psi_symbol_resolved_trees::{domain::ProofFact, expression::ExpressionNode};

    let source = r#"
        data Packet { value: u64; }
        proposition related(left: u64, right: u64) = left == right;
        domain u64::Small requires self < 10;
        machine run(hidden: Packet) {
            transition { _ -> next(hidden, [1], 0, 0) }
            state next(current: Packet, items: [u64; 1], index: u64, Small: u64)
            requires
                current.value in Small;
                items[index] in Small;
                related(current.value, items[index]);
                hidden.value in Small;
            {}
        }
    "#;
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize"))
        .expect("parse projected contract values");
    let program = lower_syntax_trees(&syntax).expect("resolve projected contract values");
    let machine = &program.machines[0];
    let state = program.machine_state(program.machine_state_handles(machine.states)[1]);
    let parameters = program.state_parameters(state.parameters);
    let contracts = program
        .tables
        .declarations
        .signature_contracts
        .span_or_empty(state.contracts);
    let facts = program.proof_facts(contracts[0].facts);
    assert_eq!(facts.len(), 4);
    for fact in facts {
        let value = match fact {
            ProofFact::Membership(membership) => {
                assert_eq!(
                    membership.domain_symbol,
                    program.domain_definitions[0].symbol
                );
                assert_ne!(membership.domain_symbol, parameters[3].symbol);
                membership.value
            }
            ProofFact::Expression(expression) => *expression,
        };
        let mut pending = vec![value];
        while let Some(expression) = pending.pop() {
            let table = &program.tables.bodies.expressions;
            match table.expression(expression) {
                ExpressionNode::Name(path) => {
                    let names = table.name_path_members(path.members);
                    let expected = parameters
                        .iter()
                        .find(|parameter| parameter.name.as_str() == names[0].as_str())
                        .map_or(psi_symbols::SymbolHandle::invalid(), |parameter| {
                            parameter.symbol
                        });
                    assert_eq!(path.head_symbol, expected, "{}", names[0].as_str());
                    if names.len() == 1 {
                        assert_eq!(path.symbol, expected, "{}", names[0].as_str());
                    }
                }
                ExpressionNode::Member(member) => pending.push(member.receiver),
                ExpressionNode::Indexed(indexed) => {
                    pending.extend([indexed.collection, indexed.index]);
                }
                ExpressionNode::Call(call) => {
                    assert_eq!(call.target_symbol, program.propositions[0].symbol);
                    pending.extend(table.expression_handles(call.arguments));
                }
                other => panic!("unexpected contract value: {other:?}"),
            }
        }
    }
}

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

#[test]
fn named_states_require_explicit_entry_value_transfers() {
    for (entry_parameters, setup) in [
        (
            "",
            "let packet: Packet = Packet { header: Header { room_id: 300 } };",
        ),
        (", packet: Packet", ""),
    ] {
        for forwarded in [false, true] {
            let (arguments, parameters, read_name) = if forwarded {
                ("packet", ", selected: Packet", "selected")
            } else {
                ("", "", "packet")
            };
            let source = format!(
                r#"
                data Header {{ room_id: u32; }}
                data Packet {{ header: Header; }}
                data Main {{}}
                machine Main::main(&mut self{entry_parameters}) {{
                    {setup}
                    transition {{ _ -> check_packet({arguments}) }}
                    state check_packet(&mut self{parameters}) {{
                        transition {read_name}.header.room_id == 300 {{ true -> {{}} _ -> {{}} }}
                    }}
                }}
            "#
            );
            let tokens = Lexer::new(&source)
                .tokenize()
                .expect("tokenize state value frontier");
            let syntax = parse_syntax_trees(&tokens).expect("parse state value frontier");
            let program = lower_syntax_trees(&syntax).expect("resolve state value frontier");
            let machine = program
                .machines
                .iter()
                .find(|machine| machine.name.as_str() == "Main::main")
                .expect("machine");
            let states = program.machine_state_handles(machine.states);
            let entry = program.machine_state(states[0]);
            let target = program.machine_state(states[1]);
            let source_symbol = program
                .state_parameters(entry.parameters)
                .iter()
                .find(|parameter| parameter.name.as_str() == "packet")
                .map(|parameter| parameter.symbol)
                .or_else(|| {
                    program
                        .tables
                        .bodies
                        .statements
                        .statements(entry.statement_nodes)
                        .iter()
                        .find_map(|statement| match statement {
                            psi_symbol_resolved_trees::statement::StatementNode::LocalData(
                                local,
                            ) if local.name.as_str() == "packet" => Some(local.symbol),
                            _ => None,
                        })
                })
                .expect("entry declares packet");
            assert!(source_symbol.is_valid());

            // The read uses a distinct name when forwarded so it cannot be
            // mistaken for the entry's transition argument expression.
            let read = program
                .tables
                .bodies
                .expressions
                .iter_expressions()
                .find_map(|(_, node)| match node {
                    psi_symbol_resolved_trees::expression::ExpressionNode::Name(path)
                        if program
                            .tables
                            .bodies
                            .expressions
                            .name_path_members(path.members)
                            .iter()
                            .map(psi_symbol_resolved_trees::name::DiagnosticName::as_str)
                            .eq([read_name]) =>
                    {
                        Some(path)
                    }
                    _ => None,
                })
                .expect("target reads the packet root");
            if forwarded {
                let parameter = program
                    .state_parameters(target.parameters)
                    .iter()
                    .find(|parameter| parameter.name.as_str() == "selected")
                    .expect("explicit target parameter");
                assert!(parameter.symbol.is_valid());
                assert_ne!(parameter.symbol, source_symbol);
                assert_eq!(read.symbol, parameter.symbol);
                assert_eq!(read.head_symbol, parameter.symbol);
            } else {
                assert!(
                    !read.symbol.is_valid(),
                    "a declared entry value is not an ambient state binding"
                );
                assert!(
                    !read.head_symbol.is_valid(),
                    "the root must not retain the entry's identity"
                );
            }
        }
    }
}
