use super::super::*;

#[test]
fn write_only_fixed_byte_range_call_retains_exact_window() {
    let source = r#"
        machine fill(bytes: &write [u8; 4]) {
            bytes[1..3] = [7, 8];
        }

        machine forward(bytes: &write [u8; 4]) {
            fill(&write bytes);
        }
    "#;

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let bytes_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "bytes")
        .map(|parameter| parameter.symbol)
        .expect("forward byte parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let mut cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &mut cache,
    );

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, psi_facts::PlaceRoot::Symbol(bytes_symbol));
    assert_eq!(
        places[0].segments,
        [psi_facts::PlaceSegment::FixedRange { start: 1, end: 3 }]
    );
}

#[test]
fn write_only_fixed_byte_call_retains_exact_literal_index() {
    let source = r#"
        machine fill(bytes: &write [u8; 4]) {
            bytes[2] = 7;
        }

        machine forward(bytes: &write [u8; 4]) {
            fill(&write bytes);
        }
    "#;

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let bytes_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "bytes")
        .map(|parameter| parameter.symbol)
        .expect("forward byte parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let mut cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &mut cache,
    );

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, psi_facts::PlaceRoot::Symbol(bytes_symbol));
    assert_eq!(
        places[0].segments,
        [psi_facts::PlaceSegment::FixedIndex { index: 2 }]
    );
}

#[test]
fn write_only_dynamic_byte_call_retains_collection_coarse_mutation() {
    let source = r#"
        machine fill(bytes: &write [u8; 4], index: u64 [0..=3]) {
            bytes[index] = 7;
        }

        machine forward(bytes: &write [u8; 4], index: u64 [0..=3]) {
            fill(&write bytes, index);
        }
    "#;

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let bytes_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "bytes")
        .map(|parameter| parameter.symbol)
        .expect("forward byte parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let mut cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &mut cache,
    );

    assert_eq!(places.len(), 1, "coarse callee write: {places:?}");
    assert_eq!(places[0].root, psi_facts::PlaceRoot::Symbol(bytes_symbol));
    assert!(
        matches!(
            places[0].segments.as_slice(),
            [psi_facts::PlaceSegment::Index { .. }]
        ),
        "a dynamic index must retain a runtime-index segment, which overlap and frame analysis conservatively treat as collection-wide: {places:?}"
    );
}

#[test]
fn write_only_record_field_call_retains_exact_common_field() {
    let source = r#"
        data Pair {
            left: u8;
            right: u16;
        }

        machine fill(pair: &write Pair) {
            pair.left = 7;
        }

        machine forward(pair: &write Pair) {
            fill(&write pair);
        }
    "#;

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let pair = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Pair")
        .expect("Pair definition");
    let left_symbol = program
        .data_members(pair)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if field.name.as_str() == "left" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("Pair.left field");
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let pair_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "pair")
        .map(|parameter| parameter.symbol)
        .expect("forward pair parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let mut cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &mut cache,
    );

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, psi_facts::PlaceRoot::Symbol(pair_symbol));
    assert_eq!(
        places[0].segments,
        [psi_facts::PlaceSegment::Field {
            symbol: left_symbol
        }]
    );
}

#[test]
fn write_only_nested_record_field_call_retains_exact_common_field_path() {
    let source = r#"
        data Inner {
            value: u8;
            spare: u8;
        }

        data Outer {
            inner: Inner;
            other: Inner;
        }

        machine fill(outer: &write Outer) {
            outer.inner.value = 7;
        }

        machine forward(outer: &write Outer) {
            fill(&write outer);
        }
    "#;

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let outer = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("Outer definition");
    let inner_field_symbol = program
        .data_members(outer)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if field.name.as_str() == "inner" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("Outer.inner field");
    let inner = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Inner")
        .expect("Inner definition");
    let value_field_symbol = program
        .data_members(inner)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if field.name.as_str() == "value" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("Inner.value field");
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let outer_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "outer")
        .map(|parameter| parameter.symbol)
        .expect("forward outer parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let mut cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &mut cache,
    );

    assert_eq!(places.len(), 1, "exact nested callee write: {places:?}");
    assert_eq!(places[0].root, psi_facts::PlaceRoot::Symbol(outer_symbol));
    assert_eq!(
        places[0].segments,
        [
            psi_facts::PlaceSegment::Field {
                symbol: inner_field_symbol,
            },
            psi_facts::PlaceSegment::Field {
                symbol: value_field_symbol,
            },
        ]
    );
}

#[test]
fn call_mutated_places_include_mutable_attached_data_arguments() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let target_symbol = SymbolHandle::from_arena_index(3);
    let player_symbol = SymbolHandle::from_arena_index(4);

    let mut program = psi_typed_trees::TypedTrees::default();
    let self_name = Expression::Name(NamePath::resolved(
        vec![Identifier::generated("self")],
        machine_symbol,
        machine_symbol,
    ));
    let player_member =
        Expression::Member(Box::new(psi_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: player_symbol,
            member: Identifier::generated("player"),
            case_variant: None,
        }));
    let player_argument = mutable_borrow(player_member);
    let player_argument = program.expression_table.insert_tree(&player_argument);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, player_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        blocks: false,
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            source_span: psi_source::SourceSpan::default(),
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: Identifier::generated("heal"),
            machine_arguments: Box::default(),
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut machine, state);
    let mut target_state = State {
        symbol: target_symbol,
        name: Identifier::generated("heal"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(5),
            name: Identifier::generated("self"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: true,
        },
    );
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(6),
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut machine, target_state);
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        machine_symbol,
        state_symbol,
        &facts,
        &call,
        &mut state_mutation_summary_cache,
    );

    assert!(places.iter().any(
        |place| place.root == psi_facts::PlaceRoot::Symbol(player_symbol)
            && place.segments.is_empty()
    ));
}

#[test]
fn call_mutated_places_include_mutable_local_arguments_from_unresolved_names() {
    let machine_symbol = SymbolHandle::from_arena_index(10);
    let state_symbol = SymbolHandle::from_arena_index(11);
    let target_symbol = SymbolHandle::from_arena_index(12);
    let local_symbol = SymbolHandle::from_arena_index(13);

    let mut program = psi_typed_trees::TypedTrees::default();
    let local_name = Expression::Name(NamePath::unresolved(vec![Identifier::generated("player")]));
    let local_argument = mutable_borrow(local_name);
    let local_argument = program.expression_table.insert_tree(&local_argument);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, local_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        blocks: false,
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(psi_typed_trees::statement::TableLocalData {
            symbol: local_symbol,
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            initial_value: psi_typed_trees::expression::ExpressionHandle::invalid(),
            is_mutable: false,
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            source_span: psi_source::SourceSpan::default(),
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: Identifier::generated("heal"),
            machine_arguments: Box::default(),
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut machine, state);
    let mut target_state = State {
        symbol: target_symbol,
        name: Identifier::generated("heal"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(14),
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut machine, target_state);
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        machine_symbol,
        state_symbol,
        &facts,
        &call,
        &mut state_mutation_summary_cache,
    );

    assert!(places.iter().any(
        |place| place.root == psi_facts::PlaceRoot::Symbol(local_symbol)
            && place.segments.is_empty()
    ));
}
