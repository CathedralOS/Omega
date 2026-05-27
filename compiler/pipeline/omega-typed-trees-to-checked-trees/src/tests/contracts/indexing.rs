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
        name: Identifier::generated("Main::main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        terminates: false,
        decreases: Default::default(),
        decrease_order: Default::default(),
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
        name: Identifier::generated("Main::main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        terminates: false,
        decreases: Default::default(),
        decrease_order: Default::default(),
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
        name: Identifier::generated("Console"),
        requires: Default::default(),
        machines: Default::default(),
    };
    let mut signature = StateSignature {
        symbol: signature_symbol,
        name: Identifier::generated("write_line"),
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
        name: Identifier::generated("Target"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        terminates: false,
        decreases: Default::default(),
        decrease_order: Default::default(),
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
            name: Identifier::generated("run"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Caller"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        terminates: false,
        decreases: Default::default(),
        decrease_order: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    let mut receiver = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut receiver, Identifier::generated("target"));
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: target_machine_symbol,
            target_symbol: target_state_symbol,
            receiver,
            target: Identifier::generated("run"),
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
        name: Identifier::generated("Drawable"),
        requires: Default::default(),
        machines: Default::default(),
    };
    let mut signature = StateSignature {
        symbol: signature_symbol,
        name: Identifier::generated("draw"),
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
        name: Identifier::generated("Sprite"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        terminates: false,
        decreases: Default::default(),
        decrease_order: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_trait_conformance(
        &mut target_machine,
        TraitConformance {
            symbol: trait_symbol,
            name: Identifier::generated("Drawable"),
        },
    );
    program.push_machine_state(
        &mut target_machine,
        State {
            symbol: target_state_symbol,
            name: Identifier::generated("draw"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        terminates: false,
        decreases: Default::default(),
        decrease_order: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    let mut receiver = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut receiver, Identifier::generated("sprite"));
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: target_machine_symbol,
            target_symbol: target_state_symbol,
            receiver,
            target: Identifier::generated("draw"),
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
        name: Identifier::generated("Main::main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        terminates: false,
        decreases: Default::default(),
        decrease_order: Default::default(),
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
        name: Identifier::generated("main"),
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
