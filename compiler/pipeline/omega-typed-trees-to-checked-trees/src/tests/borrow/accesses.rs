use super::super::*;

#[test]
fn collects_mutable_attached_data_argument_access_roots() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let target_symbol = SymbolHandle::from_arena_index(3);
    let player_symbol = SymbolHandle::from_arena_index(4);

    let mut program = omega_typed_trees::TypedTrees::default();
    let self_name = Expression::Name(NamePath::resolved(
        vec![ProgramName::generated("self")],
        machine_symbol,
        machine_symbol,
    ));
    let player_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: player_symbol,
            member: ProgramName::generated("player"),
        },
    ));
    let player_argument = Expression::Mutable(Box::new(player_member));
    let player_argument = program.expression_table.insert_tree(&player_argument);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, player_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: ProgramName::generated("heal"),
            arguments,
        }),
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine_state(
        &mut machine,
        State {
            symbol: target_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let accesses = facts.argument_accesses.span(call.accesses).unwrap();

    assert_eq!(accesses.len(), 1);
    assert_eq!(accesses[0].root_symbol, player_symbol);
    assert!(
        facts
            .access_segments
            .span_or_empty(accesses[0].segments)
            .is_empty()
    );
    assert_eq!(accesses[0].kind, BorrowAccessKind::Mutable);
}

#[test]
fn collects_disjoint_member_access_segments() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let target_symbol = SymbolHandle::from_arena_index(3);
    let player_symbol = SymbolHandle::from_arena_index(4);
    let health_symbol = SymbolHandle::from_arena_index(5);
    let stamina_symbol = SymbolHandle::from_arena_index(6);

    let mut program = omega_typed_trees::TypedTrees::default();
    let self_name = Expression::Name(NamePath::resolved(
        vec![ProgramName::generated("self")],
        machine_symbol,
        machine_symbol,
    ));
    let player_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: player_symbol,
            member: ProgramName::generated("player"),
        },
    ));
    let health_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: player_member.clone(),
            member_symbol: health_symbol,
            member: ProgramName::generated("health"),
        },
    ));
    let stamina_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: player_member,
            member_symbol: stamina_symbol,
            member: ProgramName::generated("stamina"),
        },
    ));
    let health_argument = program
        .expression_table
        .insert_tree(&Expression::Mutable(Box::new(health_member)));
    let stamina_argument = program.expression_table.insert_tree(&stamina_member);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, health_argument);
    program
        .statement_table
        .push_expression_handle(&mut arguments, stamina_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: ProgramName::generated("heal"),
            arguments,
        }),
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine_state(
        &mut machine,
        State {
            symbol: target_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let accesses = facts.argument_accesses.span(call.accesses).unwrap();

    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].root_symbol, player_symbol);
    assert_eq!(
        facts.access_segments.span_or_empty(accesses[0].segments),
        &[omega_facts::PlaceSegment::Field {
            symbol: health_symbol
        }]
    );
    assert_eq!(accesses[1].root_symbol, player_symbol);
    assert_eq!(
        facts.access_segments.span_or_empty(accesses[1].segments),
        &[omega_facts::PlaceSegment::Field {
            symbol: stamina_symbol
        }]
    );
}

#[test]
fn collects_mutable_local_borrow_loans() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.use_value(&mut self.value);
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 1;
        }
    "#;

    let tokens = omega_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = omega_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");

    let facts = build_borrow_facts(&typed);
    let borrow_state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let loans = facts.loans.span_or_empty(borrow_state.loans);
    assert_eq!(loans.len(), 1);
    assert_eq!(facts.loan_segments(&loans[0]).len(), 0);
}

#[test]
fn collects_helper_returned_mutable_local_borrow_loans() {
    let source = r#"
        data Exit {
            destination: i32;
        }

        data Room {
            exits: [Exit; 1];
        }

        data Main {
            room: Room;
        }

        machine Main::main(&mut self) {
            let alias: &mut [Exit] = self.room.exits.as_mut_slice();
        }
    "#;

    let tokens = omega_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = omega_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");

    let facts = build_borrow_facts(&typed);
    let borrow_state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let loans = facts.loans.span_or_empty(borrow_state.loans);
    assert_eq!(loans.len(), 1);
    assert_eq!(facts.loan_segments(&loans[0]).len(), 1);
}
