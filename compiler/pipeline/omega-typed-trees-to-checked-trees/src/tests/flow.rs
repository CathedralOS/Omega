use super::*;

#[test]
fn builds_shared_flow_facts_for_state_and_call_sites() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(40);
    let caller_state_symbol = SymbolHandle::from_arena_index(41);
    let callee_machine_symbol = SymbolHandle::from_arena_index(42);
    let callee_state_symbol = SymbolHandle::from_arena_index(43);

    let mut program = omega_typed_trees::TypedTrees::default();
    let contract_expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let contract_fact =
        program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(
                contract_expression,
            ));

    let callee_state = State {
        symbol: callee_state_symbol,
        name: ProgramName::generated("run"),
        parameters: Default::default(),
        return_type: Default::default(),
        statement_nodes: Default::default(),
    };
    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: ProgramName::generated("Worker::run"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine_contract(
        &mut callee_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::from_parts(contract_fact, 1),
            token_count: 1,
        },
    );
    program.push_machine(callee_machine);

    let call_arguments = HandleSpan::empty();
    let call_statement_receiver = HandleSpan::empty();
    let call_statement = StatementNode::Call(TableCall {
        receiver: call_statement_receiver,
        receiver_symbol: caller_machine_symbol,
        target: ProgramName::generated("run"),
        target_symbol: callee_state_symbol,
        arguments: call_arguments,
    });
    let caller_statement = program.statement_table.insert(call_statement);
    let caller_state = State {
        symbol: caller_state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: Default::default(),
        statement_nodes: HandleSpan::from_parts(caller_statement, 1),
    };
    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: ProgramName::generated("Main::main"),
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

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let effects = omega_effects::infer_effects(&program);
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &semantic);
    let flow = build_flow_facts(&program, &borrow, &proof, &semantic, &domains, &effects);

    let caller_flow = flow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == caller_machine_symbol
                && state.state_symbol == caller_state_symbol)
                .then_some(state)
        })
        .expect("caller flow state");
    assert!(caller_flow.entry_semantic_contexts.is_empty());
    assert_eq!(flow.calls.span_or_empty(caller_flow.calls).len(), 1);

    let call_flow = flow.calls.span_or_empty(caller_flow.calls)[0].clone();
    assert_eq!(call_flow.statement_index, 0);
    assert_eq!(call_flow.call_ordinal, 0);
    assert_eq!(call_flow.target_symbol, callee_state_symbol);
    assert!(call_flow.entry_semantic_contexts.is_empty());
    assert!(!call_flow.requires_contexts.is_empty());
    assert!(call_flow.exit_semantic_contexts.is_empty());
    assert_eq!(
        proof
            .contract_fact_refs
            .span_or_empty(call_flow.requires)
            .len(),
        1
    );
}

#[test]
fn invalidates_proved_domain_membership_after_mutating_call() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Valid {
            self.health >= 0;
            self.health <= 100;
        }

        data Main {
            player: Player;
        }

        machine Main::mark_valid(&mut self, player: &mut Player)
        ensures
            player in Player::Valid
        {
            player.health = 0;
        }

        machine Main::break_valid(&mut self, player: &mut Player) {
            player.health = 200;
        }

        machine Main::heal(&mut self, player: &mut Player)
        requires
            player in Player::Valid
        {
            player.health = 10;
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.player);
            self.break_valid(&mut self.player);
            self.heal(&mut self.player);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);
    let main_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let main_state = typed
        .machine_states(main_machine)
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("main state");
    let caller_flow = flow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol
                && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let calls = flow.calls.span_or_empty(caller_flow.calls);
    assert_eq!(calls.len(), 3);
    assert_eq!(flow.invalidations.span_or_empty(caller_flow.invalidations).len(), 1);
    assert_eq!(flow.invalidations.span_or_empty(calls[1].invalidations).len(), 1);

    let heal_call = &calls[2];
    let (required_place, required_domain) = flow
        .semantic_context_refs
        .span_or_empty(heal_call.requires_contexts)
        .iter()
        .find_map(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .facts()
                .find_map(|fact| match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                        let FactPlace::Place(place) = fact.place else {
                            return None;
                        };
                        Some((place, domain_symbol))
                    }
                    _ => None,
                })
        })
        .expect("heal requires domain membership");

    let mark_exit_proves = flow
        .semantic_context_refs
        .span_or_empty(calls[0].exit_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            context_proves_requirement_place_domain(
                &typed,
                &semantic,
                context,
                required_place,
                required_domain,
            )
        });
    let break_entry_proves = flow
        .semantic_context_refs
        .span_or_empty(calls[1].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            context_proves_requirement_place_domain(
                &typed,
                &semantic,
                context,
                required_place,
                required_domain,
            )
        });
    let heal_entry_proves = flow
        .semantic_context_refs
        .span_or_empty(calls[2].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            context_proves_requirement_place_domain(
                &typed,
                &semantic,
                context,
                required_place,
                required_domain,
            )
        });

    let diagnostics =
        lower_typed_trees(typed.clone()).expect_err("requires should fail after mutation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call heal from Main::main")
            && diagnostic
                .message
                .contains("invalidated by prior mutation of Main::main.player.health")
    }));

    assert!(mark_exit_proves);
    assert!(break_entry_proves);
    assert!(!heal_entry_proves);
}

#[test]
fn invalidates_imported_domain_requires_after_mutating_call() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Valid {
            self.health >= 0;
            self.health <= 100;
        }

        domain Player::Alive {
            self in Player::Valid;
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::mark_valid(&mut self, player: &mut Player)
        ensures
            player in Player::Valid
        {
            player.health = 0;
        }

        machine Main::break_valid(&mut self, player: &mut Player) {
            player.health = 200;
        }

        machine Main::heal(&mut self, player: &mut Player)
        requires
            player in Player::Valid
        ensures
            player in Player::Alive
        {
            player.health = 10;
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.player);
            self.break_valid(&mut self.player);
            self.heal(&mut self.player);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);
    let main_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let main_state = typed
        .machine_states(main_machine)
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("main state");
    let caller_flow = flow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol
                && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let calls = flow.calls.span_or_empty(caller_flow.calls);
    assert_eq!(flow.invalidations.span_or_empty(caller_flow.invalidations).len(), 1);
    assert_eq!(flow.invalidations.span_or_empty(calls[1].invalidations).len(), 1);
    let heal_call = &calls[2];
    let (required_place, required_domain) = flow
        .semantic_context_refs
        .span_or_empty(heal_call.requires_contexts)
        .iter()
        .find_map(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .facts()
                .find_map(|fact| match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                        let FactPlace::Place(place) = fact.place else {
                            return None;
                        };
                        Some((place, domain_symbol))
                    }
                    _ => None,
                })
        })
        .expect("heal requires domain membership");
    let heal_entry_proves = flow
        .semantic_context_refs
        .span_or_empty(calls[2].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            context_proves_requirement_place_domain(
                &typed,
                &semantic,
                context,
                required_place,
                required_domain,
            )
        });
    assert!(!heal_entry_proves);

    let diagnostics =
        lower_typed_trees(typed).expect_err("requires should fail after mutation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call heal from Main::main")
            && diagnostic
                .message
                .contains("invalidated by prior mutation of Main::main.player.health")
    }));
}

#[test]
fn preserves_imported_domain_requires_across_disjoint_mutating_call() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
            stamina: i32;
        }

        domain Player::Valid {
            self.health >= 0;
            self.health <= 100;
        }

        domain Player::Ready {
            self in Player::Valid;
            self.mana >= 0;
        }

        data Main {
            player: Player;
        }

        machine Main::mark_ready(&mut self, player: &mut Player)
        ensures
            player in Player::Ready
        {
            player.health = 40;
            player.mana = 5;
        }

        machine Main::spend_stamina(&mut self, player: &mut Player) {
            player.stamina = 0;
        }

        machine Main::heal(&mut self, player: &mut Player)
        requires
            player in Player::Ready
        {
            player.health = 50;
        }

        machine Main::main(&mut self) {
            self.mark_ready(&mut self.player);
            self.spend_stamina(&mut self.player);
            self.heal(&mut self.player);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);
    let main_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let main_state = typed
        .machine_states(main_machine)
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("main state");
    let caller_flow = flow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol
                && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let calls = flow.calls.span_or_empty(caller_flow.calls);
    assert!(flow.invalidations.span_or_empty(calls[1].invalidations).is_empty());
    let heal_call = &calls[2];
    let (required_place, required_domain) = flow
        .semantic_context_refs
        .span_or_empty(heal_call.requires_contexts)
        .iter()
        .find_map(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .facts()
                .find_map(|fact| match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                        let FactPlace::Place(place) = fact.place else {
                            return None;
                        };
                        Some((place, domain_symbol))
                    }
                    _ => None,
                })
        })
        .expect("heal requires domain membership");
    let heal_entry_proves = flow
        .semantic_context_refs
        .span_or_empty(calls[2].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            context_proves_requirement_place_domain(
                &typed,
                &semantic,
                context,
                required_place,
                required_domain,
            )
        });

    assert!(heal_entry_proves);
    lower_typed_trees(typed).expect("disjoint mutation should preserve imported domain fact");
}

#[test]
fn materializes_domain_dependency_facts() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        domain Player::Valid {
            self.health >= 0;
            self.health <= 100;
        }

        domain Player::Ready {
            self in Player::Valid;
            self.mana >= 0;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);

    let ready_symbol = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Ready")
        .map(|domain| domain.symbol)
        .expect("ready domain");
    let ready_fact = domains
        .dependencies
        .iter()
        .find_map(|(_, fact)| (fact.domain_symbol == ready_symbol).then_some(fact))
        .expect("ready dependency fact");

    let paths = domains.dependency_paths.span_or_empty(ready_fact.dependencies);
    assert_eq!(paths.len(), 2);

    let mut field_symbols = paths
        .iter()
        .filter_map(|path| {
            let segments = domains.segments.span_or_empty(path.segments);
            match segments {
                [omega_facts::PlaceSegment::Field { symbol }] => Some(*symbol),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    field_symbols.sort_by_key(|symbol| symbol.arena_index());

    let player = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Player")
        .expect("player data");
    let mut expected = typed
        .data_members(player)
        .iter()
        .filter_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == "health" || field.name.as_str() == "mana" =>
            {
                Some(field.symbol)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|symbol| symbol.arena_index());

    assert_eq!(field_symbols, expected);
}
