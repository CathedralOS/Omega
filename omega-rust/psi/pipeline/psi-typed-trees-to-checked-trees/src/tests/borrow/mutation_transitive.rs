use super::super::*;

#[test]
fn boundary_storage_fallback_keeps_receiver_and_exclusive_argument_reach() {
    let source = r#"
        data Carrier { value: &mut u64; }
        boundary trait Device {
            machine fill(value: &mut u64);
            machine consume(carrier: Carrier);
        }
        data Main { device: Device; value: u64; }
        machine Main::good(&mut self) { self.device.fill(&mut self.value); }
        machine Main::bad(&mut self) {
            self.device.consume(Carrier { value: &mut self.value });
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
    let main = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("Main");
    let fields: Vec<_> = program
        .data_members(main)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => Some(field.symbol),
            _ => None,
        })
        .collect();
    let facts = build_borrow_facts(&program);
    let mut cache = StateMutationSummaryCache::default();
    for name in ["Main::good", "Main::bad"] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("caller");
        let state = program.machine_states(machine).first().expect("entry");
        let borrow_state = facts
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|candidate| candidate.state_symbol == state.symbol)
            .expect("borrow state");
        let [call] = facts.calls.span_or_empty(borrow_state.calls) else {
            panic!("one boundary call")
        };
        let writes = call_mutated_places(
            &program,
            machine.symbol,
            state.symbol,
            &facts,
            call,
            &mut cache,
        );
        if name == "Main::bad" {
            assert!(
                writes.is_none(),
                "receiver-only fallback must not hide reference-carrier reach"
            );
            continue;
        }
        let writes = writes.expect("declared boundary frame");
        assert_eq!(writes.len(), 2, "receiver and argument storage");
        let receiver = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .expect("self");
        for &symbol in &fields {
            assert!(
                writes.contains(&crate::flow::CanonicalPlace {
                    root: psi_facts::PlaceRoot::Symbol(receiver.symbol),
                    segments: vec![psi_facts::PlaceSegment::Field { symbol }],
                }),
                "{name}: expected field {symbol:?} on {:?}, actual {writes:?}",
                receiver.symbol
            );
        }
    }
}

#[test]
fn opaque_call_fallback_rebases_known_aliases_and_rejects_unknown_prefixes() {
    let source = r#"
        data Pair { value: u64; }
        machine Pair::opaque(&mut self) { unknown(Pair { value: 0 }); }
        machine exercise(pair: &mut Pair) {
            let receiver: &mut Pair = pair;
            receiver.opaque();
            unknown(Pair { value: 0 });
            receiver.opaque();
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
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("exercise");
    let state = program.machine_states(machine).first().expect("entry");
    let pair = program
        .state_parameters(state)
        .first()
        .expect("pair")
        .symbol;
    let psi_typed_trees::statement::StatementNode::LocalData(receiver) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("receiver local")
    };
    let opaque = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Pair::opaque")
        .expect("opaque");
    let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
    assert!(
        !resolver
            .inferred_state_write_frame(opaque, &program.machine_states(opaque)[0])
            .is_complete()
    );
    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| candidate.state_symbol == state.symbol)
        .expect("borrow state");
    let [first, _unknown, second] = facts.calls.span_or_empty(borrow_state.calls) else {
        panic!("two receiver calls around an unknown prefix")
    };
    let mut cache = StateMutationSummaryCache::default();
    let first_writes = call_mutated_places(
        &program,
        machine.symbol,
        state.symbol,
        &facts,
        first,
        &mut cache,
    );
    assert_eq!(
        first_writes,
        Some(vec![crate::flow::CanonicalPlace {
            root: psi_facts::PlaceRoot::Symbol(pair),
            segments: Vec::new(),
        }])
    );
    let second_writes = call_mutated_places(
        &program,
        machine.symbol,
        state.symbol,
        &facts,
        second,
        &mut cache,
    );
    assert!(
        second_writes.is_none(),
        "an unknown prefix must not publish a disjoint local root"
    );
    for call in [first, second] {
        let accesses = crate::flow::call_write_accesses(
            &program,
            machine.symbol,
            state.symbol,
            &facts,
            call,
            &mut cache,
        );
        assert_eq!(
            accesses,
            [crate::flow::CanonicalPlace {
                root: psi_facts::PlaceRoot::Symbol(receiver.symbol),
                segments: Vec::new(),
            }]
        );
    }
}

#[test]
fn local_receiver_origins_survive_direct_and_transitive_mutation_frames() {
    let source = r#"
        data Pair { left: u16; right: u16; }
        machine Pair::write(&mut self) -> u16 { self.left = 7; 7 }
        machine Pair::relay(&mut self) {
            let receiver: &mut Pair = &mut self;
            receiver.write();
        }
        machine exercise(pair: &mut Pair, other: &mut Pair) {
            let receiver: &mut Pair = pair;
            receiver.write();
            let value: u16 = receiver.write();
            pair.relay();
            let mut selected: &mut Pair = pair;
            let prior: &mut Pair = selected;
            selected = other;
            prior.write();
            selected.write();
        }
        data Holder { pair: Pair; cells: [Pair; 2]; }
        machine Holder::relay(&mut self) {
            let receiver: &mut Pair = &mut self.pair;
            receiver.write();
        }
        machine Holder::indexed(&mut self) {
            let receiver: &mut Pair = &mut self.cells[0];
            receiver.write();
        }
        machine private() {
            let pair: Pair = Pair { left: 0, right: 0 };
            let receiver: &mut Pair = &mut pair;
            receiver.write();
        }
        machine outer(holder: &mut Holder) {
            holder.relay();
            holder.indexed();
            private();
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
    let facts = build_borrow_facts(&program);
    let mut cache = StateMutationSummaryCache::default();
    for (name, expected_paths) in [
        (
            "exercise",
            vec![
                "pair.left",
                "pair.left",
                "pair.left",
                "pair.left",
                "other.left",
            ],
        ),
        ("outer", vec!["holder.pair.left", "holder.cells", ""]),
    ] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("caller");
        let state = program.machine_states(machine).first().expect("entry");
        let borrow_state = facts
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|candidate| candidate.state_symbol == state.symbol)
            .expect("borrow state");
        let calls = facts.calls.span_or_empty(borrow_state.calls);
        assert_eq!(calls.len(), expected_paths.len());
        for (index, (call, path)) in calls.iter().zip(expected_paths).enumerate() {
            let expected = if path.is_empty() {
                Vec::new()
            } else {
                let mut members = path.split('.');
                let root = members.next().expect("root");
                let parameter = program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.name.as_str() == root)
                    .expect("parameter");
                let segments = members
                    .map(|name| {
                        let symbol = program
                            .data_definitions()
                            .iter()
                            .flat_map(|definition| program.data_members(definition))
                            .find_map(|member| match member {
                                psi_typed_trees::data::DataMember::Field(field)
                                    if field.name.as_str() == name =>
                                {
                                    Some(field.symbol)
                                }
                                _ => None,
                            })
                            .expect("field");
                        psi_facts::PlaceSegment::Field { symbol }
                    })
                    .collect();
                vec![crate::flow::CanonicalPlace {
                    root: psi_facts::PlaceRoot::Symbol(parameter.symbol),
                    segments,
                }]
            };
            let writes = call_mutated_places(
                &program,
                machine.symbol,
                state.symbol,
                &facts,
                call,
                &mut cache,
            )
            .expect("complete storage frame");
            assert_eq!(writes, expected, "{name} call {index}");
            let mut expected_accesses = expected;
            if name == "exercise" {
                let local_name = match index {
                    0 | 1 => Some("receiver"),
                    3 => Some("prior"),
                    4 => Some("selected"),
                    _ => None,
                };
                if let Some(local_name) = local_name {
                    let local_symbol = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .find_map(|statement| {
                            let psi_typed_trees::statement::StatementNode::LocalData(local) =
                                statement
                            else {
                                return None;
                            };
                            (local.name.as_str() == local_name).then_some(local.symbol)
                        })
                        .expect("access route local");
                    expected_accesses[0].root = psi_facts::PlaceRoot::Symbol(local_symbol);
                }
            }
            let accesses = crate::flow::call_write_accesses(
                &program,
                machine.symbol,
                state.symbol,
                &facts,
                call,
                &mut cache,
            );
            assert_eq!(accesses, expected_accesses, "{name} access {index}");
        }
    }
}

#[test]
fn transitive_internal_frames_distinguish_exact_and_empty_may_write_sets() {
    let source = r#"
        data Pair {
            left: u16;
            right: u16;
        }

        machine fill(pair: &write Pair) {
            pair.left = 7;
        }

        machine relay(pair: &write Pair) {
            fill(&write pair);
        }

        machine observe(pair: &mut Pair) {}

        machine exercise(pair: &write Pair, untouched: &mut Pair) {
            relay(&write pair);
            observe(&mut untouched);
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
    let exercise = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("exercise machine");
    let exercise_state = program
        .machine_states(exercise)
        .first()
        .expect("exercise entry state");
    let pair_symbol = program
        .state_parameters(exercise_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "pair")
        .map(|parameter| parameter.symbol)
        .expect("exercise pair parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == exercise_state.symbol)
        .expect("exercise borrow state");
    let [relay_call, observe_call] = facts.calls.span_or_empty(borrow_state.calls) else {
        panic!("exercise should retain exactly two calls")
    };
    let mut cache = StateMutationSummaryCache::default();
    let relay_writes = call_mutated_places(
        &program,
        exercise.symbol,
        exercise_state.symbol,
        &facts,
        relay_call,
        &mut cache,
    )
    .expect("complete storage frame");
    let observe_writes = call_mutated_places(
        &program,
        exercise.symbol,
        exercise_state.symbol,
        &facts,
        observe_call,
        &mut cache,
    )
    .expect("complete storage frame");

    assert_eq!(relay_writes.len(), 1, "transitive frame: {relay_writes:?}");
    assert_eq!(
        relay_writes[0],
        crate::flow::CanonicalPlace {
            root: psi_facts::PlaceRoot::Symbol(pair_symbol),
            segments: vec![psi_facts::PlaceSegment::Field {
                symbol: left_symbol
            }],
        }
    );
    assert!(
        observe_writes.is_empty(),
        "a complete empty body must not widen to its mutable argument: {observe_writes:?}"
    );
}

#[test]
fn bijective_recursive_frame_reaches_its_finite_fixed_point() {
    let source = r#"
        machine rotate(first: &mut u64, second: &mut u64) {
            transition { _ -> cycle(first, second) }

            state cycle(left: &mut u64, right: &mut u64) {
                left = 1;
                transition { _ -> cycle(right, left) }
            }
        }

        machine exercise(first: &mut u64, second: &mut u64) {
            rotate(first, second);
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
    let exercise = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("exercise machine");
    let exercise_state = program
        .machine_states(exercise)
        .first()
        .expect("exercise entry state");
    let parameter_symbols = program
        .state_parameters(exercise_state)
        .iter()
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == exercise_state.symbol)
        .expect("exercise borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("rotate call");
    let mut cache = StateMutationSummaryCache::default();
    let writes = call_mutated_places(
        &program,
        exercise.symbol,
        exercise_state.symbol,
        &facts,
        call,
        &mut cache,
    )
    .expect("complete storage frame");

    assert_eq!(writes.len(), 2, "finite permutation frame: {writes:?}");
    for symbol in parameter_symbols {
        assert!(writes.iter().any(|write| {
            write.root == psi_facts::PlaceRoot::Symbol(symbol) && write.segments.is_empty()
        }));
    }
}
