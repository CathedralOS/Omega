use super::*;

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
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &mut semantic, &domains, &effects);
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
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(
                    &typed,
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
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(
                    &typed,
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
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(
                    &typed,
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
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &mut semantic, &domains, &effects);
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
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(
                    &typed,
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
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &mut semantic, &domains, &effects);
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
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(
                    &typed,
                    required_place,
                    required_domain,
                )
        });

    assert!(heal_entry_proves);
    lower_typed_trees(typed).expect("disjoint mutation should preserve imported domain fact");
}
