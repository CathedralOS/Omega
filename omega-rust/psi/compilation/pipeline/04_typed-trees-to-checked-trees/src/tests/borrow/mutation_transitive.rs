use crate::borrow::build_borrow_facts;
use crate::tests::StateMutationSummaryCache;
use crate::tests::call_mutated_places;
use crate::tests::front_end::typed_program;

#[test]
fn state_handoff_closes_local_view_of_reference_bearing_parent() {
    let source = r#"
        data Room { name: &[u8]; }
        data Context { rooms: [Room; 3]; }
        machine update(context: &mut Context) {
            let rooms: &mut [Room] = context.rooms.as_mut_slice();
            rooms[1].name = "expected";
            transition rooms[0].name == "expected" {
                true -> finish(context)
                false -> finish(context)
            }
            state finish(context: &mut Context) {
                context.rooms[2].name = "finished";
            }
        }
    "#;
    let mut checked = crate::tests::front_end::checked_program_result(source)
        .expect("the local slice closes before the incoming parent reaches its successor");
    let loan = checked
        .facts
        .borrow
        .loans
        .iter()
        .find(|(_, loan)| checked.typed.symbols.name(loan.owner_symbol) == "rooms")
        .expect("the local view retains its source loan")
        .0;
    let weakening = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .iter()
        .find(|(_, weakening)| weakening.loan == loan)
        .expect("exact scope closure")
        .0;
    checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get_mut(weakening)
        .source = checked_trees::FlowInvalidationSource::Statement { statement_index: 0 };
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect_err("another closure coordinate cannot release this view at its edge");
}

#[test]
fn state_handoff_does_not_hide_local_loan_in_parent_field() {
    let source = r#"
        data Context { item: u64; held: &u64; }
        machine update(context: &mut Context) {
            let held: &u64 = &context.item;
            context.held = held;
            transition held == 0 {
                true -> finish(context)
                false -> finish(context)
            }
            state finish(context: &mut Context) {
                context.item = 1;
                let observed: u64 = context.held;
            }
        }
    "#;
    crate::tests::front_end::checked_program_result(source)
        .expect_err("the parent still carries the local loan into a conflicting successor");
}

#[test]
fn state_handoff_preserves_caller_backed_store() {
    let source = r#"
        data Context { item: u64; held: &u64; }
        machine update(context: &mut Context, caller_value: &u64) {
            context.held = caller_value;
            let observed: &u64 = &context.item;
            transition observed == 0 {
                true -> finish(context)
                false -> finish(context)
            }
            state finish(context: &mut Context) {
                context.item = 1;
                let observed: u64 = context.held;
            }
        }
    "#;
    crate::tests::front_end::checked_program_result(source)
        .expect("the stored caller loan outlives the state-local observation");
}

#[test]
fn state_handoff_does_not_hide_local_loan_through_parent_alias() {
    let source = r#"
        data Context { item: u64; held: &u64; }
        machine update(context: &mut Context) {
            let alias: &mut Context = &mut context;
            let held: &u64 = &alias.item;
            alias.held = held;
            transition held == 0 {
                true -> finish(context)
                false -> finish(context)
            }
            state finish(context: &mut Context) {
                context.item = 1;
                let observed: u64 = context.held;
            }
        }
    "#;
    crate::tests::front_end::checked_program_result(source)
        .expect_err("ending the alias cannot retire the loan stored in its parent");
}

#[test]
fn state_handoff_rejects_parent_field_backed_by_dead_local() {
    let source = r#"
        data Context { held: &u64; }
        machine update(context: &mut Context) {
            let local: u64 = 7;
            context.held = &local;
            transition { _ -> finish(context) }
            state finish(context: &mut Context) {
                let observed: u64 = context.held;
            }
        }
    "#;
    let diagnostics = crate::tests::front_end::checked_program_result(source)
        .expect_err("disjoint local backing still expires before the successor");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("whose storage ends at the source state exit")),
        "{diagnostics:?}"
    );
}

#[test]
fn state_handoff_retains_local_backing_through_owned_loan_carriers() {
    for (setup, carrier, parameter, observe) in [
        (
            "let view: &Counter = &local;",
            "view",
            "view: &Counter",
            "let observed: u64 = view.value;",
        ),
        (
            "let view: &mut Counter = &mut local;",
            "view",
            "view: &mut Counter",
            "view.value = 9;",
        ),
        (
            "let carrier: Carrier = Carrier { held: &local };",
            "carrier",
            "carrier: Carrier",
            "let observed: u64 = carrier.held.value;",
        ),
        (
            "let carrier: Envelope<Carrier> = Envelope { item: Carrier { held: &local } };",
            "carrier",
            "carrier: Envelope<Carrier>",
            "let observed: u64 = carrier.item.held.value;",
        ),
    ] {
        let source = format!(
            r#"
            data Counter {{ value: u64; }}
            data Carrier {{ held: &Counter; }}
            data Envelope<T> {{ item: T; }}
            machine update() {{
                let local: Counter = Counter {{ value: 7 }};
                {setup}
                transition {{ _ -> finish({carrier}) }}
                state finish({parameter}) {{ {observe} }}
            }}
            "#
        );
        crate::tests::front_end::checked_program_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{setup}: {diagnostics:?}"));
    }
}

#[test]
fn state_handoff_retains_local_array_backing_through_view_binding() {
    let source = r#"
        data Counter { value: u64; }
        data Room { counters: [Counter; 2]; }
        machine update() {
            let room: Room = Room {
                counters: [Counter { value: 7 }, Counter { value: 8 }]
            };
            let view: &mut [Counter] = room.counters.as_mut_slice();
            transition { _ -> finish(view) }
            state finish(view: &mut [Counter]) {
                view[0].value = 9;
            }
        }
    "#;
    crate::tests::front_end::checked_program_result(source)
        .expect("a whole view binding transfers its local backing with the loan");
}

#[test]
fn state_handoff_rejects_local_backing_stored_through_parent_alias() {
    let source = r#"
        data Context { held: &u64; }
        machine update(context: &mut Context) {
            let local: u64 = 7;
            let alias: &mut Context = &mut context;
            alias.held = &local;
            transition { _ -> finish(context) }
            state finish(context: &mut Context) {
                let observed: u64 = context.held;
            }
        }
    "#;
    let diagnostics = crate::tests::front_end::checked_program_result(source)
        .expect_err("an alias does not make caller-owned storage activation-owned");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("whose storage ends at the source state exit")),
        "{diagnostics:?}"
    );
}

#[test]
fn state_handoff_carrier_custody_tracks_reference_crossings() {
    use checked_trees::BorrowLoanOwnerSegment::{DynamicIndex, Field, FixedIndex};
    let program = typed_program(
        "data Carrier { held: &u64; }
         data Context { slots: [Carrier; 2]; }
         data Wrapper { context: &mut Context; }
         machine examine(owned: Context, borrowed: &mut Context, wrapped: Wrapper) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "examine")
        .expect("fixture machine");
    let state = &program.machine_states(machine)[0];
    let field = |owner: &str, name: &str| {
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == owner)
            .expect("declared carrier");
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                    Some(Field(field.symbol))
                }
                _ => None,
            })
            .expect("declared field")
    };
    let held = field("Carrier", "held");
    let slots = field("Context", "slots");
    let context = field("Wrapper", "context");
    for (owner, path, owned) in [
        ("borrowed", vec![], true),
        ("owned", vec![slots, FixedIndex(0), held], true),
        ("owned", vec![slots, DynamicIndex, held], true),
        ("borrowed", vec![slots, FixedIndex(0), held], false),
        ("wrapped", vec![context], true),
        ("wrapped", vec![context, slots, FixedIndex(0), held], false),
        ("wrapped", vec![context, slots, DynamicIndex, held], false),
    ] {
        let parameter = program
            .state_parameters(state)
            .iter()
            .find(|parameter| program.symbols.name(parameter.symbol) == owner)
            .expect("declared parameter");
        assert_eq!(
            crate::flow::loan_carrier_has_owned_storage(
                &program,
                state.symbol,
                0,
                parameter.symbol,
                &path,
            ),
            owned,
            "{owner}: {path:?}",
        );
    }
}

#[test]
fn state_handoff_rejects_local_backing_stored_through_owned_wrapper() {
    let source = r#"
        data Context { held: &u64; }
        data Wrapper { context: &mut Context; }
        machine update(context: &mut Context) {
            let local: u64 = 7;
            let wrapper: Wrapper = Wrapper { context: &mut context };
            wrapper.context.held = &local;
            transition { _ -> finish(context) }
            state finish(context: &mut Context) {
                let observed: u64 = context.held;
            }
        }
    "#;
    let diagnostics = crate::tests::front_end::checked_program_result(source)
        .expect_err("owned wrapper storage cannot confer ownership on its borrowed referent");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("whose storage ends at the source state exit")),
        "{diagnostics:?}"
    );
}

#[test]
fn state_handoff_retires_replaced_parent_field_loan() {
    for assignments in [
        "context.held = &context.item; context.held = caller_value;",
        "let alias: &mut Context = &mut context; alias.held = &alias.item; alias.held = caller_value;",
    ] {
        let source = format!(
            r#"
            data Context {{ item: u64; held: &u64; }}
            machine update(context: &mut Context, caller_value: &u64) {{
                {assignments}
                transition {{ _ -> finish(context) }}
                state finish(context: &mut Context) {{ context.item = 1; }}
            }}
        "#
        );
        crate::tests::front_end::checked_program_result(&source)
            .expect("replacing the field ends the old carried loan before the edge");
    }
}

#[test]
fn observation_loan_ends_before_explicit_context_state_handoff() {
    let source = "data Color [copy] { case Red; case Blue; }
        data Context { items: [Color; 3]; cursor: u64; }
        machine update(context: &mut Context) {
            transition context.cursor < 3 {
                true -> classify(context)
                _ -> stop()
            }
            state classify(context: &mut Context) {
                transition context.items[context.cursor] {
                    Color::Red -> write(context)
                    _ -> stop()
                }
            }
            state write(context: &mut Context) { context.items[0] = Color::Blue; }
            state stop() { }
        }";
    let mut checked = crate::tests::front_end::checked_program_result(source)
        .expect("the observation closes before the named successor executes");
    let resource = checked
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .next()
        .expect("retained observation closure")
        .0;
    checked
        .facts
        .borrow
        .direct_loan_resources
        .get_mut(resource)
        .weakening_source = checked_trees::FlowInvalidationSource::Statement { statement_index: 0 };
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("substituted closure boundaries must not authorize the successor");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("resource closure drifted")),
        "{diagnostics:?}"
    );
}

#[test]
fn state_handoff_does_not_retire_a_carried_observation() {
    let source = "data Context { item: u64; }
        machine update(context: &mut Context) {
            let held: &u64 = &context.item;
            transition held == 0 {
                true -> write(context, held)
                _ -> stop()
            }
            state write(context: &mut Context, held: &u64) {
                context.item = 1;
                let observed: u64 = held;
            }
            state stop() { }
        }";
    let diagnostics = crate::tests::front_end::checked_program_result(source)
        .expect_err("a carried loan remains live during the successor's mutation");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("while local borrow `held` is still active")),
        "{diagnostics:?}"
    );
}

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
