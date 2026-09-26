use crate::borrow::build_borrow_facts;
use crate::tests::front_end::typed_program;
use crate::tests::{StateMutationSummaryCache, call_mutated_places};

#[test]
fn write_only_immutable_local_range_bounds_retain_exact_element_window() {
    let source = r#"
        machine fill(values: &write [u16; 4]) {
            let start: u64 = 1;
            let start_alias: u64 = start;
            let end: u64 = 3;
            let end_alias: u64 = end;
            values[start_alias..end_alias] = [7, 8];
        }

        machine forward(values: &write [u16; 4]) {
            fill(&write values);
        }
    "#;

    let program = typed_program(source);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let values_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .map(|parameter| parameter.symbol)
        .expect("forward array parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, facts::PlaceRoot::Symbol(values_symbol));
    assert_eq!(
        places[0].segments,
        [facts::PlaceSegment::FixedRange { start: 1, end: 3 }]
    );
}

#[test]
fn write_only_fixed_copy_record_range_call_retains_exact_element_window() {
    let source = r#"
        data Leaf [copy] { value: u16; enabled: bool; }

        machine fill(values: &write [Leaf; 4], first: Leaf, second: Leaf) {
            values[1..3] = [first, second];
        }

        machine forward(values: &write [Leaf; 4], first: Leaf, second: Leaf) {
            fill(&write values, first, second);
        }
    "#;

    let program = typed_program(source);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let values_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .map(|parameter| parameter.symbol)
        .expect("forward copy-record-array parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, facts::PlaceRoot::Symbol(values_symbol));
    assert_eq!(
        places[0].segments,
        [facts::PlaceSegment::FixedRange { start: 1, end: 3 }]
    );
}

#[test]
fn write_only_fixed_copy_sum_range_call_retains_atomic_element_window() {
    let source = r#"
        data Choice [copy] {
            case Empty;
            case Value(value: u16);
        }

        machine fill(values: &write [Choice; 4], first: Choice, second: Choice) {
            values[1..3] = [first, second];
        }

        machine forward(values: &write [Choice; 4], first: Choice, second: Choice) {
            fill(&write values, first, second);
        }
    "#;

    let program = typed_program(source);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let values_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .map(|parameter| parameter.symbol)
        .expect("forward copy-sum-array parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, facts::PlaceRoot::Symbol(values_symbol));
    assert_eq!(
        places[0].segments,
        [facts::PlaceSegment::FixedRange { start: 1, end: 3 }],
        "copy-sum elements remain atomic element ordinals without case/payload segments"
    );
}

#[test]
fn write_only_nested_fixed_array_range_call_retains_atomic_outer_window() {
    let source = r#"
        machine fill(values: &write [[u16; 2]; 4], first: [u16; 2], second: [u16; 2]) {
            values[1..3] = [first, second];
        }

        machine forward(values: &write [[u16; 2]; 4], first: [u16; 2], second: [u16; 2]) {
            fill(&write values, first, second);
        }
    "#;

    let program = typed_program(source);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let values_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .map(|parameter| parameter.symbol)
        .expect("forward nested-array parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, facts::PlaceRoot::Symbol(values_symbol));
    assert_eq!(
        places[0].segments,
        [facts::PlaceSegment::FixedRange { start: 1, end: 3 }],
        "nested arrays remain atomic outer element ordinals"
    );
}

#[test]
fn write_only_fixed_copy_record_call_retains_exact_literal_index() {
    let source = r#"
        data Leaf [copy] { value: u16; enabled: bool; }

        machine fill(values: &write [Leaf; 4], replacement: Leaf) {
            values[2] = replacement;
        }

        machine forward(values: &write [Leaf; 4], replacement: Leaf) {
            fill(&write values, replacement);
        }
    "#;

    let program = typed_program(source);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let values_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .map(|parameter| parameter.symbol)
        .expect("forward copy-record-array parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert_eq!(places.len(), 1, "exact callee write: {places:?}");
    assert_eq!(places[0].root, facts::PlaceRoot::Symbol(values_symbol));
    assert_eq!(
        places[0].segments,
        [facts::PlaceSegment::FixedIndex { index: 2 }]
    );
}

#[test]
fn write_only_dynamic_copy_record_call_retains_collection_coarse_mutation() {
    let source = r#"
        data Leaf [copy] { value: u16; enabled: bool; }

        machine fill(values: &write [Leaf; 4], replacement: Leaf, index: u64 [0..=3]) {
            values[index] = replacement;
        }

        machine forward(values: &write [Leaf; 4], replacement: Leaf, index: u64 [0..=3]) {
            fill(&write values, replacement, index);
        }
    "#;

    let program = typed_program(source);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let values_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .map(|parameter| parameter.symbol)
        .expect("forward copy-record-array parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert_eq!(places.len(), 1, "coarse callee write: {places:?}");
    assert_eq!(places[0].root, facts::PlaceRoot::Symbol(values_symbol));
    assert!(
        matches!(
            places[0].segments.as_slice(),
            [facts::PlaceSegment::Index { .. }]
        ),
        "a dynamic index must retain a runtime-index segment, which overlap and frame analysis conservatively treat as collection-wide: {places:?}"
    );
}

#[test]
fn write_only_dynamic_byte_slice_call_retains_collection_coarse_mutation() {
    let source = r#"
        machine fill(bytes: &write [u8], index: u64 [0..bytes.len]) {
            bytes[index] = 7;
        }

        machine forward(bytes: &write [u8], index: u64 [0..bytes.len]) {
            fill(&write bytes, index);
        }
    "#;

    let program = typed_program(source);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let forward_state = program
        .machine_states(forward)
        .first()
        .expect("forward entry state");
    let bytes_symbol = program
        .state_parameters(forward_state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "bytes")
        .map(|parameter| parameter.symbol)
        .expect("forward byte-slice parameter");

    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == forward_state.symbol)
        .expect("forward borrow state");
    let call = facts
        .calls
        .span_or_empty(borrow_state.calls)
        .first()
        .expect("forwarding call");
    let cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        forward.symbol,
        forward_state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("complete storage frame");

    assert_eq!(places.len(), 1, "coarse slice callee write: {places:?}");
    assert_eq!(places[0].root, facts::PlaceRoot::Symbol(bytes_symbol));
    assert!(
        matches!(
            places[0].segments.as_slice(),
            [facts::PlaceSegment::Index { .. }]
        ),
        "a dynamic byte-slice store must retain the runtime-index segment that overlap and invalidation conservatively treat as collection-wide: {places:?}"
    );
}
