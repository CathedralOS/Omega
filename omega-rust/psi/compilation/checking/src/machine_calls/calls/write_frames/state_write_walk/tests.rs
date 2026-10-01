use super::{
    FrameInference, HashMap, PREFIX_WALKS, TopLevelSymbols, collect_state_write_prefixes,
    summarize_complete_state_written_paths, walk_state_write_prefix_collected,
};

const SUCCESSOR_WRITE: &str = "
    data Shared { value: &u64; }
    machine inspect(value: &mut u64, shared: Shared) {
        let alias: &mut u64 = value;
        transition { _ -> finish(alias) }
        state finish(value: &mut u64) { value = 7; }
    }
";

#[test]
fn statement_prefix_demand_stops_before_successor_effects() {
    for include_shared in [false, true] {
        let program = crate::front_end::typed_program(SUCCESSOR_WRITE);
        let mut diagnostics = Vec::new();
        let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
        assert!(diagnostics.is_empty());
        let machine = symbols.machine("inspect").unwrap();
        let state = &program.machine_states(machine)[0];

        PREFIX_WALKS.with(|walks| walks.set(0));
        let prefixes =
            collect_state_write_prefixes(&program, machine, state, &symbols, include_shared)
                .unwrap();
        assert_eq!(prefixes.len(), 2);
        assert!(prefixes[0].as_ref().unwrap().aliases.is_empty());
        let stored = &prefixes[0].as_ref().unwrap().stored;
        if include_shared {
            assert_eq!(stored.len(), 1);
            assert_eq!(stored[0].references.len(), 1);
            assert_eq!(stored[0].references[0].origin.path, "shared.value");
        } else {
            assert!(stored.is_empty());
        }
        let aliases = &prefixes[1].as_ref().unwrap().aliases;
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].0, "alias");
        assert_eq!(aliases[0].1.path, "value");
        assert_eq!(
            PREFIX_WALKS.with(|walks| walks.get()),
            1,
            "a before-statement query must not summarize successor states"
        );
        symbols
            .caller_sites
            .with_complete_state_summaries(|summaries| {
                assert!(summaries.is_empty(), "prefixes are not complete summaries");
            });
    }
}

#[test]
fn whole_state_demand_retains_successor_effects() {
    let program = crate::front_end::typed_program(SUCCESSOR_WRITE);
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    let machine = symbols.machine("inspect").unwrap();
    let state = &program.machine_states(machine)[0];
    let mut inference = FrameInference::default();
    let mut summaries = HashMap::default();

    let written = summarize_complete_state_written_paths(
        &program,
        machine,
        state,
        &symbols,
        &mut inference,
        &mut summaries,
    )
    .unwrap();
    assert_eq!(written, ["value"]);

    for include_shared in [false, true] {
        let mut summaries = HashMap::default();
        let (prefix, boundaries) = walk_state_write_prefix_collected(
            &program,
            machine,
            state,
            &symbols,
            &mut inference,
            &mut summaries,
            include_shared,
        )
        .unwrap();
        assert_eq!(prefix.written, ["value"]);
        assert_eq!(boundaries.len(), 2);
    }
}
