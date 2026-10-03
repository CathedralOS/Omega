use crate::resolution::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn state_contract_value_arguments_share_the_explicit_frontier() {
    use symbol_resolved_trees::{domain::ProofFact, expression::ExpressionNode};

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
    let program =
        resolve(ResolutionRequest::new(syntax)).expect("resolve projected contract values");
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
                        .map_or(symbols::SymbolHandle::invalid(), |parameter| {
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
        let program = resolve(ResolutionRequest::new(syntax)).expect("resolve");
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "run")
            .expect("caller");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let symbol_resolved_trees::statement::StatementNode::LocalData(local) = &program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)[0]
        else {
            panic!("result");
        };
        let symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
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
fn local_call_targets_follow_prior_declarations_not_receiver_spelling() {
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::statement::StatementNode;
    use symbols::SymbolHandle;

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
        let program = resolve(ResolutionRequest::new(syntax)).expect("resolve");
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
