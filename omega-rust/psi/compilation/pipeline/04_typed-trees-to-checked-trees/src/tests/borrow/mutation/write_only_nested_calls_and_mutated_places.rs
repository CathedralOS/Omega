use crate::borrow::build_borrow_facts;
use crate::tests::{
    Expression, HandleSpan, Identifier, Machine, NamePath, State, StateMutationSummaryCache,
    StateParameter, StatementNode, SymbolHandle, TableCall, call_mutated_places, mutable_borrow,
};

#[test]
fn call_mutated_places_include_mutable_attached_data_arguments() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let target_symbol = SymbolHandle::from_arena_index(3);
    let player_symbol = SymbolHandle::from_arena_index(4);

    let mut program = typed_trees::TypedTrees::default();
    let self_name = Expression::Name(NamePath::resolved(
        vec![Identifier::generated("self")],
        machine_symbol,
        machine_symbol,
    ));
    let player_member = Expression::Member(Box::new(checked_trees::expression::MemberExpression {
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
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        attached_data_application: typed_trees::types::TypeReferenceHandle::invalid(),
        generic_data_template: symbols::SymbolHandle::invalid(),
        spelling: None,
        is_public: false,
        supply_mode: Default::default(),
        target: None,
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        structural_type_equations_pending: false,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_root_symbol: SymbolHandle::invalid(),
            source_span: source::SourceSpan::default(),
            authored_call_selection: None,
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: Identifier::generated("heal"),
            static_machine_parameter: symbols::SymbolHandle::invalid(),
            static_requirement_dispatch: None,
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
        return_type: typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(5),
            name: Identifier::generated("self"),
            type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: true,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(6),
            name: Identifier::generated("player"),
            type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    program.push_machine_state(&mut machine, target_state);
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let state_mutation_summary_cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        machine_symbol,
        state_symbol,
        &facts,
        &call,
        &state_mutation_summary_cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert!(places.iter().any(
        |place| place.root == facts::PlaceRoot::Symbol(player_symbol) && place.segments.is_empty()
    ));
}

#[test]
fn call_mutated_places_include_mutable_local_arguments_from_unresolved_names() {
    let machine_symbol = SymbolHandle::from_arena_index(10);
    let state_symbol = SymbolHandle::from_arena_index(11);
    let target_symbol = SymbolHandle::from_arena_index(12);
    let local_symbol = SymbolHandle::from_arena_index(13);

    let mut program = typed_trees::TypedTrees::default();
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
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        attached_data_application: typed_trees::types::TypeReferenceHandle::invalid(),
        generic_data_template: symbols::SymbolHandle::invalid(),
        spelling: None,
        is_public: false,
        supply_mode: Default::default(),
        target: None,
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        structural_type_equations_pending: false,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(typed_trees::statement::TableLocalData {
            symbol: local_symbol,
            name: Identifier::generated("player"),
            type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
            initial_value: typed_trees::expression::ExpressionHandle::invalid(),
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_root_symbol: SymbolHandle::invalid(),
            source_span: source::SourceSpan::default(),
            authored_call_selection: None,
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: Identifier::generated("heal"),
            static_machine_parameter: symbols::SymbolHandle::invalid(),
            static_requirement_dispatch: None,
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
        return_type: typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(14),
            name: Identifier::generated("player"),
            type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    program.push_machine_state(&mut machine, target_state);
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let state_mutation_summary_cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        machine_symbol,
        state_symbol,
        &facts,
        &call,
        &state_mutation_summary_cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert!(
        places
            .iter()
            .any(|place| place.root == facts::PlaceRoot::Symbol(local_symbol)
                && place.segments.is_empty())
    );
}
