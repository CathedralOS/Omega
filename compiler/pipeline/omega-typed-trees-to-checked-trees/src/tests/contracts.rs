use super::*;

#[test]
fn carries_machine_contract_facts_into_checked_proof_facts() {
    let machine_symbol = SymbolHandle::from_arena_index(5);

    let mut program = omega_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(omega_typed_trees::domain::ProofFact::Expression(expression));
    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Main::main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_machine(machine);

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_fact = facts
        .contract_facts
        .iter()
        .next()
        .map(|(_, fact)| fact)
        .expect("checked proof facts should include the machine contract");

    assert_eq!(facts.contract_facts.len(), 1);
    assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
    assert_eq!(contract_fact.fact, fact);
    assert_eq!(
        contract_fact.owner,
        ContractProofFactOwner::Machine { machine_symbol }
    );
}

#[test]
fn centralizes_contract_facts_in_semantic_fact_plan() {
    let machine_symbol = SymbolHandle::from_arena_index(5);

    let mut program = omega_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(omega_typed_trees::domain::ProofFact::Expression(expression));
    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Main::main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_machine(machine);

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&program, &proof);

    assert_eq!(semantic.facts.len(), 1);
    assert_eq!(semantic.contexts.len(), 1);
    assert_eq!(semantic.symbol_sets.len(), 0);

    let semantic_fact = semantic
        .facts
        .iter()
        .next()
        .map(|(_, fact)| fact)
        .expect("semantic contract fact");
    let omega_facts::FactPlace::Place(place) = semantic_fact.place else {
        panic!("expected canonical contract fact place");
    };
    assert_eq!(
        semantic.places.get(place).root,
        omega_facts::PlaceRoot::Expression(expression)
    );
    assert_eq!(
        semantic_fact.payload,
        omega_facts::FactPayload::ContractBooleanExpression {
            kind: omega_facts::ContractFactKind::Requires,
            fact,
            expression,
        }
    );
    let context = semantic
        .contexts_at_point(omega_facts::ProgramPoint::Machine { machine_symbol })
        .next()
        .expect("machine contract context");
    assert_eq!(context.boolean_facts().count(), 1);
}

#[test]
fn instantiates_call_contract_places_onto_caller_arguments() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(1);
    let caller_state_symbol = SymbolHandle::from_arena_index(2);
    let callee_machine_symbol = SymbolHandle::from_arena_index(3);
    let callee_state_symbol = SymbolHandle::from_arena_index(4);
    let caller_argument_symbol = SymbolHandle::from_arena_index(5);
    let callee_parameter_symbol = SymbolHandle::from_arena_index(6);

    let mut program = omega_typed_trees::TypedTrees::default();
    let caller_argument_expression =
        program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Name(
                omega_typed_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: caller_argument_symbol,
                    symbol: caller_argument_symbol,
                },
            ));
    let callee_parameter_expression =
        program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Name(
                omega_typed_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: callee_parameter_symbol,
                    symbol: callee_parameter_symbol,
                },
            ));
    let callee_fact =
        program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(
                callee_parameter_expression,
            ));

    let mut caller_arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut caller_arguments, caller_argument_expression);
    let caller_statement = program
        .statement_table
        .insert(StatementNode::Call(TableCall {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: callee_state_symbol,
            receiver: HandleSpan::empty(),
            target: ProgramName::generated("run"),
            arguments: caller_arguments,
        }));

    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: ProgramName::generated("main"),
        parameters: HandleSpan::empty(),
        return_type: Default::default(),
        statement_nodes: HandleSpan::from_parts(caller_statement, 1),
    };
    program.push_state_parameter(
        &mut caller_state,
        StateParameter {
            symbol: caller_argument_symbol,
            name: ProgramName::generated("value"),
            type_reference: Default::default(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        },
    );

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: ProgramName::generated("Caller"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let mut callee_state = State {
        symbol: callee_state_symbol,
        name: ProgramName::generated("run"),
        parameters: HandleSpan::empty(),
        return_type: Default::default(),
        statement_nodes: HandleSpan::empty(),
    };
    program.push_state_parameter(
        &mut callee_state,
        StateParameter {
            symbol: callee_parameter_symbol,
            name: ProgramName::generated("amount"),
            type_reference: Default::default(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        },
    );

    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: ProgramName::generated("Worker"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine(callee_machine);

    let call = omega_checked_trees::ContractCallFact {
        caller_machine_symbol,
        caller_state_symbol,
        statement_index: 0,
        call_ordinal: 0,
        target_machine_symbol: callee_machine_symbol,
        target_state_symbol: callee_state_symbol,
        requires: HandleSpan::empty(),
        ensures: HandleSpan::empty(),
    };
    let contract = omega_checked_trees::ContractProofFact {
        kind: ContractProofFactKind::Requires,
        owner: ContractProofFactOwner::MachineState {
            machine_symbol: callee_machine_symbol,
            state_symbol: callee_state_symbol,
        },
        fact: callee_fact,
    };

    let mut semantic = omega_facts::FactPlan::default();
    let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
    let omega_facts::FactPlace::Place(place_handle) = place else {
        panic!("expected instantiated call place");
    };

    assert_eq!(
        semantic.places.get(place_handle).root,
        omega_facts::PlaceRoot::Symbol(caller_argument_symbol)
    );
}

#[test]
fn carries_trait_signature_contract_facts_into_checked_proof_facts() {
    let trait_symbol = SymbolHandle::from_arena_index(5);
    let signature_symbol = SymbolHandle::from_arena_index(6);

    let mut program = omega_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(omega_typed_trees::domain::ProofFact::Expression(expression));

    let mut trait_definition = TraitDefinition {
        symbol: trait_symbol,
        is_boundary: true,
        name: ProgramName::generated("Console"),
        requires: Default::default(),
        machines: Default::default(),
    };
    let mut signature = StateSignature {
        symbol: signature_symbol,
        name: ProgramName::generated("write_line"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        effects: Default::default(),
        contracts: Default::default(),
    };
    program.push_state_signature_contract(
        &mut signature,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_trait_machine_signature(&mut trait_definition, signature);
    program.push_trait_definition(trait_definition);

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_fact = facts
        .contract_facts
        .iter()
        .next()
        .map(|(_, fact)| fact)
        .expect("checked proof facts should include the trait signature contract");

    assert_eq!(facts.contract_facts.len(), 1);
    assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
    assert_eq!(contract_fact.fact, fact);
    assert_eq!(
        contract_fact.owner,
        ContractProofFactOwner::StateSignature {
            owner_symbol: trait_symbol,
            state_symbol: signature_symbol,
        }
    );
}

#[test]
fn indexes_call_contract_facts_by_target_machine() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(5);
    let caller_state_symbol = SymbolHandle::from_arena_index(6);
    let target_machine_symbol = SymbolHandle::from_arena_index(7);
    let target_state_symbol = SymbolHandle::from_arena_index(8);

    let mut program = omega_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(omega_typed_trees::domain::ProofFact::Expression(expression));

    let mut target_machine = Machine {
        symbol: target_machine_symbol,
        name: ProgramName::generated("Target"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_contract(
        &mut target_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_machine_state(
        &mut target_machine,
        State {
            symbol: target_state_symbol,
            name: ProgramName::generated("run"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: ProgramName::generated("Caller"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    let mut receiver = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut receiver, ProgramName::generated("target"));
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: target_machine_symbol,
            target_symbol: target_state_symbol,
            receiver,
            target: ProgramName::generated("run"),
            arguments: Default::default(),
        }),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_call = facts
        .contract_calls
        .iter()
        .next()
        .map(|(_, call)| call)
        .expect("checked proof facts should index the call contract");
    let requires = facts
        .contract_fact_refs
        .span_or_empty(contract_call.requires);

    assert_eq!(facts.contract_calls.len(), 1);
    assert_eq!(contract_call.caller_machine_symbol, caller_machine_symbol);
    assert_eq!(contract_call.caller_state_symbol, caller_state_symbol);
    assert_eq!(contract_call.statement_index, 0);
    assert_eq!(contract_call.call_ordinal, 0);
    assert_eq!(contract_call.target_machine_symbol, target_machine_symbol);
    assert_eq!(contract_call.target_state_symbol, target_state_symbol);
    assert_eq!(requires.len(), 1);
    assert_eq!(facts.contract_facts.get(requires[0].fact).fact, fact);
}

#[test]
fn indexes_inherited_trait_contracts_by_concrete_call_target() {
    let trait_symbol = SymbolHandle::from_arena_index(5);
    let signature_symbol = SymbolHandle::from_arena_index(6);
    let target_machine_symbol = SymbolHandle::from_arena_index(7);
    let target_state_symbol = SymbolHandle::from_arena_index(8);
    let caller_machine_symbol = SymbolHandle::from_arena_index(9);
    let caller_state_symbol = SymbolHandle::from_arena_index(10);

    let mut program = omega_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(omega_typed_trees::domain::ProofFact::Expression(expression));

    let mut trait_definition = TraitDefinition {
        symbol: trait_symbol,
        is_boundary: true,
        name: ProgramName::generated("Drawable"),
        requires: Default::default(),
        machines: Default::default(),
    };
    let mut signature = StateSignature {
        symbol: signature_symbol,
        name: ProgramName::generated("draw"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        effects: Default::default(),
        contracts: Default::default(),
    };
    program.push_state_signature_contract(
        &mut signature,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_trait_machine_signature(&mut trait_definition, signature);
    program.push_trait_definition(trait_definition);

    let mut target_machine = Machine {
        symbol: target_machine_symbol,
        name: ProgramName::generated("Sprite"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_trait_conformance(
        &mut target_machine,
        TraitConformance {
            symbol: trait_symbol,
            name: ProgramName::generated("Drawable"),
        },
    );
    program.push_machine_state(
        &mut target_machine,
        State {
            symbol: target_state_symbol,
            name: ProgramName::generated("draw"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: ProgramName::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    let mut receiver = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut receiver, ProgramName::generated("sprite"));
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: target_machine_symbol,
            target_symbol: target_state_symbol,
            receiver,
            target: ProgramName::generated("draw"),
            arguments: Default::default(),
        }),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_call = facts
        .contract_calls
        .iter()
        .next()
        .map(|(_, call)| call)
        .expect("checked proof facts should index inherited trait contracts");
    let requires = facts
        .contract_fact_refs
        .span_or_empty(contract_call.requires);

    assert_eq!(facts.contract_calls.len(), 1);
    assert_eq!(requires.len(), 1);
    let inherited_fact = facts.contract_facts.get(requires[0].fact);
    assert_eq!(inherited_fact.kind, ContractProofFactKind::Requires);
    assert_eq!(inherited_fact.fact, fact);
    assert_eq!(
        inherited_fact.owner,
        ContractProofFactOwner::MachineState {
            machine_symbol: target_machine_symbol,
            state_symbol: target_state_symbol,
        }
    );
}

#[test]
fn indexes_terminal_state_contract_ensures() {
    let machine_symbol = SymbolHandle::from_arena_index(5);
    let state_symbol = SymbolHandle::from_arena_index(6);

    let mut program = omega_typed_trees::TypedTrees::default();
    let fact_expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(omega_typed_trees::domain::ProofFact::Expression(
            fact_expression,
        ));
    let return_expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Integer(0));

    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Main::main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );

    let mut state = State {
        symbol: state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(return_expression),
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let exit = facts
        .contract_exits
        .iter()
        .next()
        .map(|(_, exit)| exit)
        .expect("checked proof facts should index the exit contract");
    let ensures = facts.contract_fact_refs.span_or_empty(exit.ensures);

    assert_eq!(facts.contract_exits.len(), 1);
    assert_eq!(exit.machine_symbol, machine_symbol);
    assert_eq!(exit.state_symbol, state_symbol);
    assert_eq!(exit.statement_index, 0);
    assert_eq!(ensures.len(), 1);
    assert_eq!(facts.contract_facts.get(ensures[0].fact).fact, fact);
}

#[test]
fn instantiates_call_contract_places_for_attached_data_arguments() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(1);
    let caller_state_symbol = SymbolHandle::from_arena_index(2);
    let callee_machine_symbol = SymbolHandle::from_arena_index(3);
    let callee_state_symbol = SymbolHandle::from_arena_index(4);
    let caller_player_symbol = SymbolHandle::from_arena_index(5);
    let callee_player_symbol = SymbolHandle::from_arena_index(6);

    let mut program = omega_typed_trees::TypedTrees::default();
    let player_fact_expression =
        program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Name(
                omega_checked_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: callee_player_symbol,
                    symbol: callee_player_symbol,
                },
            ));
    let callee_fact =
        program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(
                player_fact_expression,
            ));

    let mut caller_arguments = HandleSpan::empty();
    let self_name = Expression::Name(NamePath::resolved(
        vec![ProgramName::generated("self")],
        caller_machine_symbol,
        caller_machine_symbol,
    ));
    let player_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: caller_player_symbol,
            member: ProgramName::generated("player"),
        },
    ));
    let player_argument = Expression::Mutable(Box::new(player_member));
    let player_argument = program.expression_table.insert_tree(&player_argument);
    program
        .statement_table
        .push_expression_handle(&mut caller_arguments, player_argument);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: ProgramName::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: caller_machine_symbol,
            target_symbol: callee_state_symbol,
            receiver: Default::default(),
            target: ProgramName::generated("heal"),
            arguments: caller_arguments,
        }),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: ProgramName::generated("Game"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut callee_state = State {
        symbol: callee_state_symbol,
        name: ProgramName::generated("heal"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut callee_state,
        StateParameter {
            symbol: callee_player_symbol,
            name: ProgramName::generated("player"),
            type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine(callee_machine);

    let call = omega_checked_trees::ContractCallFact {
        caller_machine_symbol,
        caller_state_symbol,
        statement_index: 0,
        call_ordinal: 0,
        target_machine_symbol: callee_machine_symbol,
        target_state_symbol: callee_state_symbol,
        requires: HandleSpan::empty(),
        ensures: HandleSpan::empty(),
    };
    let contract = omega_checked_trees::ContractProofFact {
        kind: ContractProofFactKind::Requires,
        owner: ContractProofFactOwner::MachineState {
            machine_symbol: callee_machine_symbol,
            state_symbol: callee_state_symbol,
        },
        fact: callee_fact,
    };

    let mut semantic = omega_facts::FactPlan::default();
    let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
    let omega_facts::FactPlace::Place(place_handle) = place else {
        panic!("expected instantiated call place");
    };
    let place = semantic.places.get(place_handle);
    let segments = semantic.place_segments.span_or_empty(place.segments);

    assert_eq!(
        place.root,
        omega_facts::PlaceRoot::Symbol(caller_machine_symbol)
    );
    assert_eq!(segments.len(), 1);
    assert_eq!(
        segments[0],
        omega_facts::PlaceSegment::Field {
            symbol: caller_player_symbol
        }
    );
}

#[test]
fn instantiates_call_contract_places_for_expression_statement_calls() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(1);
    let caller_state_symbol = SymbolHandle::from_arena_index(2);
    let callee_machine_symbol = SymbolHandle::from_arena_index(3);
    let callee_state_symbol = SymbolHandle::from_arena_index(4);
    let caller_player_symbol = SymbolHandle::from_arena_index(5);
    let callee_player_symbol = SymbolHandle::from_arena_index(6);

    let mut program = omega_typed_trees::TypedTrees::default();
    let player_fact_expression =
        program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Name(
                omega_checked_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: callee_player_symbol,
                    symbol: callee_player_symbol,
                },
            ));
    let callee_fact =
        program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(
                player_fact_expression,
            ));

    let self_name = Expression::Name(NamePath::resolved(
        vec![ProgramName::generated("self")],
        caller_machine_symbol,
        caller_machine_symbol,
    ));
    let player_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: caller_player_symbol,
            member: ProgramName::generated("player"),
        },
    ));
    let player_argument = Expression::Mutable(Box::new(player_member));
    let call_expression = Expression::Call(Box::new(CallExpression {
        receiver: Some(Box::new(Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            caller_machine_symbol,
            caller_machine_symbol,
        )))),
        target_symbol: callee_state_symbol,
        target: ProgramName::generated("heal"),
        arguments: Arc::from(vec![player_argument].into_boxed_slice()),
    }));
    let call_expression = program.expression_table.insert_tree(&call_expression);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: ProgramName::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Expression(call_expression),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: ProgramName::generated("Game"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut callee_state = State {
        symbol: callee_state_symbol,
        name: ProgramName::generated("heal"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut callee_state,
        StateParameter {
            symbol: callee_player_symbol,
            name: ProgramName::generated("player"),
            type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine(callee_machine);

    let call = omega_checked_trees::ContractCallFact {
        caller_machine_symbol,
        caller_state_symbol,
        statement_index: 0,
        call_ordinal: 0,
        target_machine_symbol: callee_machine_symbol,
        target_state_symbol: callee_state_symbol,
        requires: HandleSpan::empty(),
        ensures: HandleSpan::empty(),
    };
    let contract = omega_checked_trees::ContractProofFact {
        kind: ContractProofFactKind::Requires,
        owner: ContractProofFactOwner::MachineState {
            machine_symbol: callee_machine_symbol,
            state_symbol: callee_state_symbol,
        },
        fact: callee_fact,
    };

    let mut semantic = omega_facts::FactPlan::default();
    let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
    let omega_facts::FactPlace::Place(place_handle) = place else {
        panic!("expected instantiated call place");
    };
    let place = semantic.places.get(place_handle);
    let segments = semantic.place_segments.span_or_empty(place.segments);

    assert_eq!(
        place.root,
        omega_facts::PlaceRoot::Symbol(caller_machine_symbol)
    );
    assert_eq!(segments.len(), 1);
    assert_eq!(
        segments[0],
        omega_facts::PlaceSegment::Field {
            symbol: caller_player_symbol
        }
    );
}
