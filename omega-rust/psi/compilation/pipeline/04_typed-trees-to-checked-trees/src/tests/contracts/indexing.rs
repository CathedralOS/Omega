use super::super::{
    HandleSpan, Identifier, Machine, SignatureContract, SignatureContractKind, State,
    StatementNode, SymbolHandle, TableCall,
};

use crate::CheckingRequest;
use crate::borrow::build_borrow_facts;
use crate::lower_typed_trees;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;
use crate::tests::contracts::{parse_typed_trees, parse_typed_trees_with_service};

#[test]
fn centralizes_contract_facts_in_semantic_fact_plan() {
    let machine_symbol = SymbolHandle::from_arena_index(5);

    let mut program = typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(typed_trees::domain::ProofFact::Expression(expression));
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main::main"),
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
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_machine(machine);

    let proof_plan = proof::obligations::build_proof_plan(&program);
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
    let facts::FactPlace::Place(place) = semantic_fact.place else {
        panic!("expected canonical contract fact place");
    };
    assert_eq!(
        semantic.places.get(place).root,
        facts::PlaceRoot::Expression(expression)
    );
    assert_eq!(
        semantic_fact.payload,
        facts::FactPayload::ContractBooleanExpression {
            kind: facts::ContractFactKind::Requires,
            fact,
            expression,
            instantiated: arena::Handle::invalid(),
        }
    );
    let context = semantic
        .contexts_at_point(facts::ProgramPoint::Machine { machine_symbol })
        .next()
        .expect("machine contract context");
    assert_eq!(context.boolean_facts().count(), 1);
}

#[test]
fn indexes_call_contract_facts_by_target_machine() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(5);
    let caller_state_symbol = SymbolHandle::from_arena_index(6);
    let target_machine_symbol = SymbolHandle::from_arena_index(7);
    let target_state_symbol = SymbolHandle::from_arena_index(8);

    let mut program = typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(typed_trees::domain::ProofFact::Expression(expression));

    let mut target_machine = Machine {
        symbol: target_machine_symbol,
        name: Identifier::generated("Target"),
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
    program.push_machine_contract(
        &mut target_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
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
            return_type: typed_trees::types::TypeReferenceHandle::invalid(),
            contracts: Default::default(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Caller"),
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
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    let mut receiver = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut receiver, Identifier::generated("target"));
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_root_symbol: target_machine_symbol,
            source_span: source::SourceSpan::default(),
            authored_call_selection: None,
            receiver_symbol: target_machine_symbol,
            target_symbol: target_state_symbol,
            receiver,
            target: Identifier::generated("run"),
            static_machine_parameter: symbols::SymbolHandle::invalid(),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            arguments: Default::default(),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let proof_plan = proof::obligations::build_proof_plan(&program);
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
fn boundary_ensures_witness_wide_bounded_assignment_refuses() {
    // `self.n + 2` reaches 10 > 9 -- the witness must not over-prove.
    let source = r#"
        pub boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }
        data Main { fw: Binding<Firmware>; n: u32; m: u32 [0..=9]; }
        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.m = self.n + 2;
        }
    "#;
    let diagnostics = lower_typed_trees(
        parse_typed_trees_with_service(source),
        &CheckingRequest::settled(),
    )
    .expect_err("a fold past the target must refuse");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("satisfies bounded target `self.m`")),
        "expected the containment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn transitive_guard_dies_when_value_call_writes_guarded_place() {
    let source = r#"
        data Main {
            buf: [u8; 4];
            i: u32;
            scratch: u64;
        }

        machine Main::main(&mut self) {
            self.i = 0;
            transition self.i < 4 { true -> prepare() _ -> done() }

            state prepare(&mut self) {
                self.scratch = self.touch_i();
                transition { _ -> put() }
            }

            state touch_i(&mut self) -> u64 {
                self.i = 4;
                0
            }

            state put(&mut self) {
                self.buf[self.i] = 7;
            }

            state done(&mut self) {}
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("an overlapping value-call frame must invalidate the transitive guard");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove index `self.i` is within length 4")),
        "expected the index refusal, got {diagnostics:#?}"
    );
}

#[test]
fn indexed_write_through_an_unproven_or_moved_selector_records_nothing() {
    for body in [
        // `slot` is bounded but has no exact value, so `rooms[slot]` names
        // some element; the write cannot be attributed to any one of them.
        "rooms[slot].exit_count = 1; append_exit(&mut rooms[slot], 7);",
        // The selector was pinned at the store but moved before the call:
        // the fact correctly landed on `rooms[0]` and must not follow the
        // selector to `rooms[1]`.
        "let mut at: u64 [0..=3] = 0; rooms[at].exit_count = 1; at = 1; \
         append_exit(&mut rooms[at], 7);",
        // A pinned selector overwritten by a call-site value is unproven
        // again at the store.
        "let mut at: u64 [0..=3] = 0; at = unknown; \
         rooms[at].exit_count = 1; append_exit(&mut rooms[at], 7);",
    ] {
        let source = format!(
            r#"
            data Exit {{ destination: u32; }}
            data Room {{ exits: [Exit; 4]; exit_count: u32; }}
            machine append_exit(room: &mut Room, destination: u32)
            requires room.exit_count < 4 {{
                room.exit_count = destination;
            }}
            machine establish(slot: u64 [0..=3], unknown: u64 [0..=3]) {{
                let mut rooms: [Room; 4] = [
                    Room {{
                        exits: [
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }}
                        ],
                        exit_count: 5
                    }},
                    Room {{
                        exits: [
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }}
                        ],
                        exit_count: 5
                    }},
                    Room {{
                        exits: [
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }}
                        ],
                        exit_count: 5
                    }},
                    Room {{
                        exits: [
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }},
                            Exit {{ destination: 0 }}
                        ],
                        exit_count: 5
                    }}
                ];
                {body}
            }}
        "#
        );
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
            "{body}: {diagnostics:#?}"
        );
    }
}
