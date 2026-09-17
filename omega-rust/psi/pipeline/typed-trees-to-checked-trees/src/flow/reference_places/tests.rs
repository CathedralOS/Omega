use super::{FlowCallFact, FlowFacts, FlowStateFact, PlaceRoot, StatementNode};
use crate::flow::CanonicalPlace;
use crate::flow::reference_places::{preserve_call_prefix_storage, preserve_frame};
use facts::{NormalizedWriteFrame, PlaceSegment};

#[test]
fn operand_frames_must_preserve_both_binding_and_referent() {
    let source = "data Context { scheduler: u64; counter: u64; }
        machine observe(value: u64) -> u64 { value }
        machine probe(context: &mut Context) -> u64 {
            let mut borrowed: &Context = &context;
            transition { _ -> observe(borrowed.scheduler) }
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .unwrap();
    let typed_state = &program.machine_states(machine)[0];
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("reference declaration")
    };
    let context = program.state_parameters(typed_state)[0].symbol;
    let scheduler = program
        .data_definitions()
        .iter()
        .flat_map(|definition| program.data_members(definition))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "scheduler" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .unwrap();
    let state = FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: typed_state.symbol,
        ..Default::default()
    };
    let original = CanonicalPlace {
        root: PlaceRoot::Symbol(local.symbol),
        segments: vec![PlaceSegment::Field { symbol: scheduler }],
    };
    let referent = CanonicalPlace {
        root: PlaceRoot::Symbol(context),
        segments: original.segments.clone(),
    };
    let index = statements.len() - 1;
    let binding_write = NormalizedWriteFrame::complete(vec!["borrowed".to_owned()]);
    let frames = validation::CallFrameResolver::new(&program).expect("typed program resolves");
    // The old referent alone is disjoint from replacing the local binding.
    assert_eq!(
        preserve_frame(
            &program,
            machine,
            &state,
            index,
            std::slice::from_ref(&referent),
            &binding_write,
            &frames,
        ),
        Some(())
    );
    let places = [original, referent];
    for frame in [
        binding_write,
        NormalizedWriteFrame::complete(vec!["context.scheduler".to_owned()]),
        NormalizedWriteFrame::opaque(),
    ] {
        assert_eq!(
            preserve_frame(&program, machine, &state, index, &places, &frame, &frames),
            None
        );
    }
    assert_eq!(
        preserve_frame(
            &program,
            machine,
            &state,
            index,
            &places,
            &NormalizedWriteFrame::complete(vec!["context.counter".to_owned()]),
            &frames,
        ),
        Some(())
    );
}

/// The prefix bound for custody is the call's position in the state's
/// execution-ordered rows. That position is a recorded coordinate —
/// `(statement_index, call_ordinal)` plus target and receiver identity —
/// not the address the row happens to occupy in this arena slice. A
/// replayed copy of the same recorded row must resolve to the same bound.
#[test]
fn call_prefix_bound_replays_from_recorded_call_identity() {
    let source = "data Context { scheduler: u64; counter: u64; }
        machine observe(value: u64) -> u64 { value }
        machine probe(context: &mut Context) -> u64 {
            let mut borrowed: &Context = &context;
            transition { _ -> observe(borrowed.scheduler) }
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .unwrap();
    let observe = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let typed_state = &program.machine_states(machine)[0];
    let frames = validation::CallFrameResolver::new(&program).expect("typed program resolves");

    let mut flow = FlowFacts::default();
    // Statement 0 declares `borrowed`; statement 1 holds the transition call.
    let subject = FlowCallFact {
        statement_index: 1,
        call_ordinal: 0,
        target_symbol: observe.symbol,
        ..Default::default()
    };
    let calls = flow.control.calls.insert_many([subject.clone()]);
    let state = FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: typed_state.symbol,
        calls,
        ..Default::default()
    };
    let places: [CanonicalPlace; 0] = [];
    let arena_row = &flow.control.calls.span_or_empty(state.calls)[0];

    // The in-arena row resolves its own bound: no earlier same-statement
    // call exists, so custody is preserved.
    assert_eq!(
        preserve_call_prefix_storage(
            &program, &frames, machine, &flow, &state, arena_row, &places
        ),
        Some(())
    );
    // The same recorded row replayed by value must reach the same verdict.
    assert_eq!(
        preserve_call_prefix_storage(&program, &frames, machine, &flow, &state, &subject, &places),
        Some(())
    );
    // A recorded coordinate absent from the state's calls declines.
    let foreign = FlowCallFact {
        statement_index: 1,
        call_ordinal: 1,
        target_symbol: observe.symbol,
        ..Default::default()
    };
    assert_eq!(
        preserve_call_prefix_storage(&program, &frames, machine, &flow, &state, &foreign, &places),
        None
    );
}

/// Two rows carrying the same recorded call identity make the execution
/// bound ambiguous; the checker declines rather than borrowing a
/// same-shaped row's position.
#[test]
fn call_prefix_bound_declines_ambiguous_recorded_identity() {
    let source = "machine observe(value: u64) -> u64 { value }
        machine probe(context: u64) -> u64 {
            transition { _ -> observe(context) }
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .unwrap();
    let observe = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let typed_state = &program.machine_states(machine)[0];
    let frames = validation::CallFrameResolver::new(&program).expect("typed program resolves");

    let mut flow = FlowFacts::default();
    let subject = FlowCallFact {
        statement_index: 0,
        call_ordinal: 0,
        target_symbol: observe.symbol,
        ..Default::default()
    };
    let calls = flow
        .control
        .calls
        .insert_many([subject.clone(), subject.clone()]);
    let state = FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: typed_state.symbol,
        calls,
        ..Default::default()
    };
    let places: [CanonicalPlace; 0] = [];
    let arena_row = &flow.control.calls.span_or_empty(state.calls)[0];
    assert_eq!(
        preserve_call_prefix_storage(
            &program, &frames, machine, &flow, &state, arena_row, &places
        ),
        None
    );
}
