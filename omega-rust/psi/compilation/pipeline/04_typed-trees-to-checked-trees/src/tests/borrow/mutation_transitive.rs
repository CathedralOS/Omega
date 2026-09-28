use crate::borrow::build_borrow_facts;
use crate::tests::StateMutationSummaryCache;
use crate::tests::call_mutated_places;
use crate::tests::front_end::typed_program;

#[test]
fn direct_unknown_alias_store_does_not_become_an_empty_caller_frame() {
    assert_direct_alias_store_frame(
        "let alias: &mut u64 = opaque(&mut pair.left); alias = 7;",
        None,
    );
}

fn assert_direct_alias_store_frame(body: &str, expected_paths: Option<&[&str]>) {
    let source = format!(
        "data Pair {{ left: u64; right: u64; }}
         data Main {{ pair: Pair; other: Pair; cells: [Pair; 2]; }}
         machine opaque(value: &mut u64) -> &mut u64 {{ opaque(value) }}
         machine update(pair: &mut Pair, other: &mut Pair, cells: &mut [Pair; 2]) {{ {body} }}
         machine Main::run(&mut self) {{ update(&mut self.pair, &mut self.other, &mut self.cells); }}"
    );
    let program = typed_program(&source);
    let helper = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "update")
        .expect("helper");
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    let helper_frame =
        resolver.inferred_state_write_frame(helper, &program.machine_states(helper)[0]);
    assert_eq!(
        helper_frame.is_complete(),
        expected_paths.is_some(),
        "shared admission for {body}"
    );
    if let Some(expected_paths) = expected_paths {
        let mut expected_relative: Vec<_> = expected_paths
            .iter()
            .map(|path| {
                let path = path.split('[').next().expect("coarse frame prefix");
                let (root, suffix) = path
                    .split_once('.')
                    .map_or((path, String::new()), |(root, suffix)| {
                        (root, format!(".{suffix}"))
                    });
                let position = ["pair", "other", "cells"]
                    .iter()
                    .position(|name| *name == root)
                    .expect("helper parameter");
                format!("$P{position}{suffix}")
            })
            .collect();
        let mut actual_relative = helper_frame
            .complete_paths()
            .expect("complete shared frame")
            .to_vec();
        actual_relative.sort();
        expected_relative.sort();
        assert_eq!(
            actual_relative, expected_relative,
            "shared storage origins for {body}"
        );
    }

    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = program.machine_states(caller).first().expect("entry");
    let receiver = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .expect("self");
    let facts = build_borrow_facts(&program);
    let borrow_state = facts
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| candidate.state_symbol == state.symbol)
        .expect("borrow state");
    let [call] = facts.calls.span_or_empty(borrow_state.calls) else {
        panic!("one helper call")
    };
    let cache = StateMutationSummaryCache::default();
    let writes = call_mutated_places(
        &program,
        caller.symbol,
        state.symbol,
        &facts,
        call,
        &cache,
        ::validation::CallFrameResolver::new(&program).as_ref(),
    );
    let Some(expected_paths) = expected_paths else {
        assert!(
            writes.as_ref().is_none_or(|paths| !paths.is_empty()),
            "an opaque alias store must invalidate conservatively, not publish no writes"
        );
        return;
    };
    let writes = writes.expect("complete structured caller frame");
    let expected: Vec<_> = expected_paths
        .iter()
        .map(|path| {
            let segments = expected_segments(&program, path);
            crate::flow::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(receiver.symbol),
                segments,
            }
        })
        .collect();
    assert_eq!(writes.len(), expected.len(), "{body}: {writes:?}");
    for expected in expected {
        assert!(
            writes.contains(&expected),
            "{body}: expected {expected:?}, actual {writes:?}"
        );
    }
}

fn expected_segments(program: &typed_trees::TypedTrees, path: &str) -> Vec<facts::PlaceSegment> {
    let mut segments = Vec::new();
    for member in path.split('.').filter(|member| !member.is_empty()) {
        let (name, index) = member
            .split_once('[')
            .map_or((member, None), |(name, index)| (name, Some(index)));
        let symbol = program
            .data_definitions()
            .iter()
            .flat_map(|definition| program.data_members(definition))
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                    Some(field.symbol)
                }
                _ => None,
            })
            .expect("unique fixture field");
        segments.push(facts::PlaceSegment::Field { symbol });
        if let Some(index) = index {
            segments.push(facts::PlaceSegment::FixedIndex {
                index: index
                    .strip_suffix(']')
                    .expect("index suffix")
                    .parse()
                    .expect("literal index"),
            });
        }
    }
    segments
}

#[test]
fn shared_boundary_resolver_preserves_exact_and_opaque_storage_frames() {
    let source = r#"
        data Carrier { value: &mut u64; }
        boundary trait Device {
            machine fill(value: &mut u64);
            machine consume(carrier: Carrier);
        }
        data Main<'s> { device: &'s mut Device; value: u64; }
        machine Main::good(&mut self) { self.device.fill(&mut self.value); }
        machine Main::forward(&mut self) {
            let output: &mut u64 = &mut self.value;
            self.device.fill(output);
        }
        machine Main::bad(&mut self) {
            self.device.consume(Carrier { value: &mut self.value });
        }
    "#;
    let program = typed_program(source);
    let main = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("Main");
    let fields: Vec<_> = program
        .data_members(main)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field.symbol),
            _ => None,
        })
        .collect();
    let facts = build_borrow_facts(&program);
    let cache = StateMutationSummaryCache::default();
    let shared_frames = validation::CallFrameResolver::new(&program).expect("valid symbols");
    for name in [
        "Main::good",
        "Main::forward",
        "Main::bad",
        "Main::bad",
        "Main::forward",
        "Main::good",
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
        let [call] = facts.calls.span_or_empty(borrow_state.calls) else {
            panic!("one boundary call")
        };
        let writes = call_mutated_places(
            &program,
            machine.symbol,
            state.symbol,
            &facts,
            call,
            &cache,
            Some(&shared_frames),
        );
        let fresh = call_mutated_places(
            &program,
            machine.symbol,
            state.symbol,
            &facts,
            call,
            &StateMutationSummaryCache::default(),
            validation::CallFrameResolver::new(&program).as_ref(),
        );
        assert_eq!(writes, fresh, "sharing cannot change the frame for {name}");
        // Without a shared resolver the frame falls back to the declared
        // signature ceiling (receiver plus exclusive actuals), never to an
        // empty write set.
        assert!(
            !matches!(
                call_mutated_places(
                    &program,
                    machine.symbol,
                    state.symbol,
                    &facts,
                    call,
                    &cache,
                    None,
                ),
                Some(places) if places.is_empty()
            ),
            "an unavailable boundary resolver is not an empty write frame",
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
                    root: facts::PlaceRoot::Symbol(receiver.symbol),
                    segments: vec![facts::PlaceSegment::Field { symbol }],
                }),
                "{name}: expected field {symbol:?} on {:?}, actual {writes:?}",
                receiver.symbol
            );
        }
    }
}
