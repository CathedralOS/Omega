use super::*;

#[test]
fn direct_alias_stores_invalidate_domain_facts_but_rebinding_does_not() {
    for (assignment, invalidates) in [
        ("alias.health = 200;", true),
        ("alias = &mut self.other;", false),
    ] {
        let source = format!(
            r#"
            data Player {{ health: i32; }}
            domain Player::Valid requires self.health >= 0; self.health <= 100;
            data Main {{ player: Player; other: Player; }}
            machine Main::mark(&mut self, player: &mut Player)
                ensures player in Player::Valid {{ player.health = 0; }}
            machine Main::heal(&mut self, player: &mut Player)
                requires player in Player::Valid {{ player.health = 10; }}
            machine Main::main(&mut self) {{
                self.mark(&mut self.player);
                let alias: &mut Player = &mut self.player;
                {assignment}
                self.heal(&mut self.player);
            }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
        let operations = psi_effects::infer_operational_may(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let mut semantic = build_semantic_facts(&typed, &proof);
        let domains = build_domain_facts(&typed, &semantic);
        let flow = build_flow_facts(
            &typed,
            &borrow,
            &proof,
            &mut semantic,
            &domains,
            &operations,
        );
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("caller");
        let caller_flow = flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.machine_symbol == machine.symbol)
            .expect("caller flow");
        let state = typed.machine_states(machine).first().expect("entry");
        let receiver = typed
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .expect("self");
        let psi_typed_trees::statement::StatementNode::LocalData(alias) =
            &typed.statement_table.statements(state.statement_nodes)[1]
        else {
            panic!("alias");
        };
        let invalidations: Vec<_> = flow
            .invalidations
            .events
            .span_or_empty(caller_flow.invalidations)
            .iter()
            .filter(|event| {
                matches!(
                    event.source,
                    psi_checked_trees::FlowInvalidationSource::Statement { statement_index: 2 }
                )
            })
            .collect();
        assert_eq!(
            invalidations.len(),
            if invalidates { 2 } else { 1 },
            "{assignment}"
        );
        assert!(
            invalidations
                .iter()
                .any(|event| event.mutated_root == psi_facts::PlaceRoot::Symbol(alias.symbol))
        );
        assert_eq!(
            invalidations
                .iter()
                .any(|event| event.mutated_root == psi_facts::PlaceRoot::Symbol(receiver.symbol)),
            invalidates,
            "rebinding invalidates the alias's copied fact, not the owner's fact"
        );
        let checked = lower_typed_trees(typed);
        if invalidates {
            let diagnostics = checked.expect_err("referent write must invalidate the domain");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call heal")
                    && diagnostic
                        .message
                        .contains("invalidated by prior mutation of self.player.health")),
                "{diagnostics:?}"
            );
        } else {
            checked.expect("binding replacement does not write the old referent");
        }
    }
}

#[test]
fn invalidates_proved_domain_membership_after_mutating_call() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Valid
        requires
            self.health >= 0;
            self.health <= 100;

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
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
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
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let calls = flow.control.calls.span_or_empty(caller_flow.calls);
    assert_eq!(calls.len(), 3);
    assert_eq!(
        flow.invalidations
            .events
            .span_or_empty(caller_flow.invalidations)
            .len(),
        1
    );
    assert_eq!(
        flow.invalidations
            .events
            .span_or_empty(calls[1].invalidations)
            .len(),
        1
    );

    let heal_call = &calls[2];
    let (required_place, required_domain) = flow
        .contexts
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
        .contexts
        .semantic_context_refs
        .span_or_empty(calls[0].exit_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(&typed, required_place, required_domain)
        });
    let break_entry_proves = flow
        .contexts
        .semantic_context_refs
        .span_or_empty(calls[1].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(&typed, required_place, required_domain)
        });
    let heal_entry_proves = flow
        .contexts
        .semantic_context_refs
        .span_or_empty(calls[2].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(&typed, required_place, required_domain)
        });

    let diagnostics =
        lower_typed_trees(typed.clone()).expect_err("requires should fail after mutation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call heal from Main::main")
            && diagnostic
                .message
                .contains("invalidated by prior mutation of self.player.health")
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

        domain Player::Valid
        requires
            self.health >= 0;
            self.health <= 100;

        domain Player::Alive
        requires
            self in Player::Valid;
            self.health > 0;

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
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
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
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let calls = flow.control.calls.span_or_empty(caller_flow.calls);
    assert_eq!(
        flow.invalidations
            .events
            .span_or_empty(caller_flow.invalidations)
            .len(),
        1
    );
    assert_eq!(
        flow.invalidations
            .events
            .span_or_empty(calls[1].invalidations)
            .len(),
        1
    );
    let heal_call = &calls[2];
    let (required_place, required_domain) = flow
        .contexts
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
        .contexts
        .semantic_context_refs
        .span_or_empty(calls[2].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(&typed, required_place, required_domain)
        });
    assert!(!heal_entry_proves);

    let diagnostics = lower_typed_trees(typed).expect_err("requires should fail after mutation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call heal from Main::main")
            && diagnostic
                .message
                .contains("invalidated by prior mutation of self.player.health")
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

        domain Player::Valid
        requires
            self.health >= 0;
            self.health <= 100;

        domain Player::Ready
        requires
            self in Player::Valid;
            self.mana >= 0;

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
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
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
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let calls = flow.control.calls.span_or_empty(caller_flow.calls);
    assert!(
        flow.invalidations
            .events
            .span_or_empty(calls[1].invalidations)
            .is_empty()
    );
    let heal_call = &calls[2];
    let (required_place, required_domain) = flow
        .contexts
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
        .contexts
        .semantic_context_refs
        .span_or_empty(calls[2].entry_semantic_contexts)
        .iter()
        .any(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(&typed, required_place, required_domain)
        });

    assert!(heal_entry_proves);
    lower_typed_trees(typed).expect("disjoint mutation should preserve imported domain fact");
}

#[test]
fn preserves_domain_intersection_requires_across_unrelated_machine_field_mutation() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
            salt: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        domain Password::Secure
        requires
            self.score >= 8;

        data Main {
            password: Password;
            unrelated: i32 in Wrapping;
        }

        machine Main::mark_valid(&mut self, password: &mut Password)
        ensures
            password in Password::Valid
        {
            password.length = 12;
        }

        machine Main::mark_secure(&mut self, password: &mut Password)
        ensures
            password in Password::Secure
        {
            password.score = 10;
        }

        machine Main::bump_unrelated(&mut self) {
            self.unrelated = self.unrelated + 1;
        }

        machine Main::touch_unrelated(&mut self) {
            self.bump_unrelated();
        }

        machine Main::accept(password: Password)
        requires
            password in Password::Valid & Password::Secure
        {
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.password);
            self.mark_secure(&mut self.password);
            self.touch_unrelated();
            self.accept(self.password);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
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
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let calls = flow.control.calls.span_or_empty(caller_flow.calls);
    assert_eq!(calls.len(), 4);
    let borrow_calls = borrow.calls.span_or_empty(
        borrow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == main_machine.symbol
                    && state.state_symbol == main_state.symbol)
                    .then_some(state.calls)
            })
            .expect("borrow main calls"),
    );
    let touch_unrelated_borrow_call = &borrow_calls[2];
    let target_state = crate::find_state(&typed, touch_unrelated_borrow_call.target_symbol);
    assert!(
        target_state.is_some(),
        "expected touch_unrelated target state, got {:?}",
        touch_unrelated_borrow_call.target_symbol
    );
    let target_state = target_state.expect("target state");
    assert_eq!(target_state.name.as_str(), "touch_unrelated");
    let mut cache = StateMutationSummaryCache::default();
    let mutated_places = call_mutated_places(
        &typed,
        main_machine.symbol,
        main_state.symbol,
        &borrow,
        touch_unrelated_borrow_call,
        &mut cache,
    )
    .expect("complete storage frame");
    assert_eq!(
        mutated_places.len(),
        1,
        "mutated places: {mutated_places:?}"
    );
    assert_eq!(
        mutated_places[0].segments.len(),
        1,
        "mutated places: {mutated_places:?}"
    );
    assert!(
        flow.invalidations
            .events
            .span_or_empty(calls[2].invalidations)
            .is_empty(),
        "touch_unrelated should not invalidate password domain facts: {:?}",
        flow.invalidations
            .events
            .span_or_empty(calls[2].invalidations)
    );
    let accept_call = &calls[3];
    let required_facts: Vec<_> = flow
        .contexts
        .semantic_context_refs
        .span_or_empty(accept_call.requires_contexts)
        .iter()
        .flat_map(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .facts()
                .filter_map(|fact| match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                        let FactPlace::Place(place) = fact.place else {
                            return None;
                        };
                        Some((place, domain_symbol))
                    }
                    _ => None,
                })
        })
        .collect();
    assert_eq!(required_facts.len(), 2);
    for (required_place, required_domain) in required_facts {
        let proves = flow
            .contexts
            .semantic_context_refs
            .span_or_empty(accept_call.entry_semantic_contexts)
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
        assert!(
            proves,
            "accept entry should still prove domain {:?} for place {:?}",
            required_domain, required_place
        );
    }

    lower_typed_trees(typed)
        .expect("unrelated machine-field mutation should preserve intersection requirements");
}

#[test]
fn stores_and_calls_invalidate_domain_facts_copied_to_aliases() {
    let mut failures = Vec::new();
    for assignment in [
        "alias.health = 200;",
        "self.player.health = 200;",
        "alias.break_valid();",
    ] {
        let source = format!(
            r#"
            data Player {{ health: i32; }}
            domain Player::Valid requires self.health >= 0; self.health <= 100;
            data Main {{ player: Player; }}
            machine Player::break_valid(&mut self) {{ self.health = 200; }}
            machine Main::mark(&mut self, player: &mut Player)
                ensures player in Player::Valid {{ player.health = 0; }}
            machine Main::heal(&mut self, player: &mut Player)
                requires player in Player::Valid {{ player.health = 10; }}
            machine Main::main(&mut self) {{
                self.mark(&mut self.player);
                let alias: &mut Player = &mut self.player;
                {assignment}
                self.heal(alias);
            }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
        let operations = psi_effects::infer_operational_may(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let mut semantic = build_semantic_facts(&typed, &proof);
        let domains = build_domain_facts(&typed, &semantic);
        let flow = build_flow_facts(
            &typed,
            &borrow,
            &proof,
            &mut semantic,
            &domains,
            &operations,
        );
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("caller");
        let caller_flow = flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.machine_symbol == machine.symbol)
            .expect("caller flow");
        let calls = flow.control.calls.span_or_empty(caller_flow.calls);
        assert_eq!(
            calls.len(),
            if assignment == "alias.break_valid();" {
                3
            } else {
                2
            }
        );
        let heal = calls.last().expect("heal call");
        let required_domains: Vec<_> = flow
            .contexts
            .semantic_context_refs
            .span_or_empty(heal.requires_contexts)
            .iter()
            .flat_map(|reference| {
                semantic
                    .context_view(semantic.contexts.get(reference.context))
                    .facts()
                    .filter_map(|fact| match (fact.place, fact.payload) {
                        (
                            FactPlace::Place(place),
                            FactPayload::ContractDomainMembership { domain_symbol, .. },
                        ) => Some((place, domain_symbol)),
                        _ => None,
                    })
            })
            .collect();
        assert_eq!(
            required_domains.len(),
            1,
            "heal requires the alias's domain"
        );
        for (place, domain) in required_domains {
            let retained = flow
                .contexts
                .semantic_context_refs
                .span_or_empty(heal.entry_semantic_contexts)
                .iter()
                .any(|reference| {
                    semantic
                        .context_view(semantic.contexts.get(reference.context))
                        .proves_place_domain_membership_in_program(&typed, place, domain)
                });
            if retained {
                failures.push(format!(
                    "{assignment}: heal still receives the copied alias domain fact"
                ));
            }
        }
        match lower_typed_trees(typed) {
            Err(diagnostics)
                if diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("cannot prove requires contract for call heal")
                }) => {}
            // An owner-spelled write can fail borrow checking before the call
            // contract phase. The explicit context assertion above still pins
            // removal of the alias's copied domain fact in that case.
            Err(diagnostics)
                if assignment == "self.player.health = 200;"
                    && diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .message
                            .contains("while local borrow `alias` is still active")
                    }) => {}
            Err(diagnostics) => failures.push(format!(
                "{assignment}: missing domain rejection: {diagnostics:?}"
            )),
            Ok(_) => failures.push(format!(
                "{assignment}: checked lowering incorrectly accepts the call"
            )),
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
