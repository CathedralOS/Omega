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
