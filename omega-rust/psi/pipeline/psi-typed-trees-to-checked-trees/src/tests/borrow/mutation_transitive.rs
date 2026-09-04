use super::super::*;

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
    );
    let observe_writes = call_mutated_places(
        &program,
        exercise.symbol,
        exercise_state.symbol,
        &facts,
        observe_call,
        &mut cache,
    );

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
    );

    assert_eq!(writes.len(), 2, "finite permutation frame: {writes:?}");
    for symbol in parameter_symbols {
        assert!(writes.iter().any(|write| {
            write.root == psi_facts::PlaceRoot::Symbol(symbol) && write.segments.is_empty()
        }));
    }
}
