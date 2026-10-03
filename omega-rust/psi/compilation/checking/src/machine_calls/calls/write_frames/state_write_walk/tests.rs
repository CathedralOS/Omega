use super::{
    FrameInference, PREFIX_WALKS, TopLevelSymbols, collect_state_write_prefixes,
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
fn collection_length_referent_store_retains_its_write_frame_across_calls() {
    let program = crate::front_end::typed_program(
        "data Main {}
         machine Main::count(seen: &mut u64, sub: &[i32]) { seen = sub.len; }
         machine Main::elements(seen: &mut u64, items: &[i32], lo: u64, hi: u64) {
             transition lo <= hi && hi <= items.len {
                 true -> Main::count(seen, items[lo..hi])
                 false -> skipped(seen)
             }
             state skipped(seen: &mut u64) { seen = 0; }
         }",
    );
    let resolver = crate::CallFrameResolver::new(&program).expect("resolver");
    for machine in program.machines() {
        let frames = resolver.inferred_machine_state_write_frames(machine);
        for (state, frame) in program.machine_states(machine).iter().zip(frames) {
            assert_eq!(
                frame.complete_paths(),
                Some(["$P0".to_owned()].as_slice()),
                "{}::{}",
                machine.name,
                state.name,
            );
            assert_eq!(resolver.inferred_state_write_frame(machine, state), frame);
        }
    }
}

#[test]
fn borrowed_array_record_replacement_has_a_storage_write_frame() {
    let program = crate::front_end::typed_program(
        "data Entry { observed: &u64; value: u64; }
         machine write(entries: &mut [Entry; 2], observed: &u64, position: u64)
         requires position < 2;
         {
             entries[position] = Entry { observed: observed, value: 7 };
         }",
    );
    let machine = &program.machines()[0];
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[0].complete_paths(),
        Some(["$P0".to_owned()].as_slice())
    );
}

#[test]
fn borrowed_array_record_replacement_composes_through_a_named_cycle() {
    let program = crate::front_end::typed_program(
        "data Entry { observed: &u64; value: u64; }
         machine write(entries: &mut [Entry; 2], observed: &u64) {
             transition { _ -> fill(entries, observed, 0) }
             state fill(entries: &mut [Entry; 2], observed: &u64, position: u64)
             requires position < 2;
             {
                 entries[position] = Entry { observed: observed, value: 7 };
                 transition position < 1 {
                     true -> fill(entries, observed, position + 1)
                     _ -> done(entries)
                 }
             }
             state done(entries: &mut [Entry; 2]) {}
         }",
    );
    let machine = &program.machines()[0];
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[0].complete_paths(),
        Some(["$P0".to_owned()].as_slice())
    );
}

#[test]
fn borrowed_slice_record_replacement_has_a_storage_write_frame() {
    let program = crate::front_end::typed_program(
        "data Entry { observed: &u64; value: u64; }
         machine write(entries: &mut [Entry], observed: &u64, position: u64)
         requires position < entries.len;
         {
             entries[position] = Entry { observed: observed, value: 7 };
         }",
    );
    let machine = &program.machines()[0];
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[0].complete_paths(),
        Some(["$P0".to_owned()].as_slice())
    );
}

#[test]
fn indexed_replacement_cannot_preserve_stale_mutable_reference_origins() {
    let program = crate::front_end::typed_program(
        "data Entry { destination: &mut u64; }
         machine write(entries: &mut [Entry; 2], replacement: &mut u64, position: u64)
         requires position < 2;
         {
             entries[position] = Entry { destination: replacement };
             entries[position].destination = 7;
         }",
    );
    let machine = &program.machines()[0];
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    if let Some(paths) = frames[0].complete_paths() {
        assert!(
            paths.contains(&"$P1".to_owned()),
            "lost replacement destination: {paths:?}"
        );
    }
}

#[test]
fn indexed_replacement_retains_reference_slots_introduced_by_a_new_case() {
    let program = crate::front_end::typed_program(
        "data Entry { case Empty; case Live(destination: &mut u64); }
         machine write(replacement: &mut u64) {
             let mut entries: [Entry; 2] = [Entry::Empty, Entry::Empty];
             entries[0] = Entry::Live { destination: replacement };
             let observed: u64 = 7;
         }",
    );
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let prefixes = collect_state_write_prefixes(&program, machine, state, &symbols, false)
        .expect("prefix collection");
    let stored = &prefixes[2].as_ref().expect("post-assignment prefix").stored;
    let entry = stored
        .iter()
        .find(|local| program.symbols.name(local.local_symbol) == "entries")
        .expect("array frontier");
    assert!(
        !entry.references.is_empty(),
        "new case must not lose its reference slot"
    );
    assert!(entry.references.iter().all(|leaf| leaf.origin.is_none()));
}

#[test]
fn private_metadata_writes_do_not_require_unrelated_reference_origins() {
    let program = crate::front_end::typed_program(
        "data Entry { observed: &u64; value: u64; }
         data Carrier { entries: &mut [Entry]; count: u64; }
         machine make(entries: &mut [Entry]) -> Carrier {
             Carrier { entries: entries, count: 0 }
         }
         machine Carrier::set_count(&mut self) { self.count = 7; }
         machine update(entries: &mut [Entry]) {
             let mut carrier: Carrier = make(entries);
             carrier.set_count();
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "update")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(frames[0].complete_paths(), Some([].as_slice()));
}

#[test]
fn unresolved_carrier_referent_writes_cannot_become_private() {
    let program = crate::front_end::typed_program(
        "data Entry { observed: &u64; value: u64; }
         data Carrier { entries: &mut [Entry]; count: u64; }
         machine make(entries: &mut [Entry]) -> Carrier {
             Carrier { entries: entries, count: 0 }
         }
         machine update(entries: &mut [Entry]) {
             let mut carrier: Carrier = make(entries);
             carrier.entries[0].value = 7;
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "update")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    if let Some(paths) = frames[0].complete_paths() {
        assert!(paths.contains(&"entries".to_owned()), "{paths:?}");
    }
}

#[test]
fn unresolved_reference_frontier_survives_local_and_call_transport() {
    let program = crate::front_end::typed_program(
        "data Entry { observed: &u64; value: u64; }
         data Carrier { entries: &mut [Entry]; count: u64; }
         machine make(entries: &mut [Entry]) -> Carrier {
             Carrier { entries: entries, count: 0 }
         }
         machine relocate(value: Carrier) -> Carrier {
             let moved: Carrier = value;
             moved
         }
         machine Carrier::set_count(&mut self) { self.count = 7; }
         machine metadata(entries: &mut [Entry]) {
             let original: Carrier = make(entries);
             let mut carrier: Carrier = relocate(original);
             carrier.set_count();
         }
         machine referent(entries: &mut [Entry]) {
             let original: Carrier = make(entries);
             let mut carrier: Carrier = relocate(original);
             carrier.entries[0].value = 7;
         }",
    );
    let resolver = crate::CallFrameResolver::new(&program).expect("resolver");
    for machine in program.machines() {
        match machine.name.as_str() {
            "metadata" => {
                let frames = resolver.inferred_machine_state_write_frames(machine);
                assert_eq!(frames[0].complete_paths(), Some([].as_slice()));
            }
            "referent" => {
                let frames = resolver.inferred_machine_state_write_frames(machine);
                if let Some(paths) = frames[0].complete_paths() {
                    assert!(paths.contains(&"entries".to_owned()), "{paths:?}");
                }
            }
            _ => {}
        }
    }
}

#[test]
fn returned_carrier_preserves_origins_after_parameter_reborrow() {
    let program = crate::front_end::typed_program(
        "data Carrier { destination: &mut u64; count: u64; }
         machine initialize(destination: &mut u64) { destination = 7; }
         machine make(destination: &mut u64) -> Carrier {
             initialize(&mut destination);
             Carrier { destination: destination, count: 0 }
         }
         data Root { scratch: u64; }
         machine Root::write(&mut self) {
             let carrier: Carrier = make(&mut self.scratch);
             carrier.destination = 11;
         }",
    );
    let _scope = crate::enter_frozen_program_scope(&program);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::write")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[0].complete_paths(),
        Some(["self.scratch".to_owned()].as_slice())
    );

    // The resolver has solved initialize's complete write frame. An origin
    // query must still derive make's returned relation, but not re-walk that
    // already completed callee merely because it is beneath a result query.
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    let state = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("carrier initializer");
    };
    PREFIX_WALKS.with(|walks| walks.set(0));
    let origins = super::super::path_instantiation::aggregate_arguments::reference_leaves(
        &program,
        machine,
        local.initial_value,
        local.type_reference,
        "",
        &symbols,
        &mut FrameInference::default(),
    )
    .expect("body-derived returned origin");
    assert_eq!(origins.references.len(), 1);
    assert_eq!(
        origins.references[0].origin.as_ref().unwrap().path,
        "self.scratch"
    );
    assert_eq!(
        PREFIX_WALKS.with(|walks| walks.get()),
        1,
        "derive the result relation without rebuilding completed callee effects"
    );
    let make = symbols.machine("make").expect("result producer");
    let make_state = &program.machine_states(make)[0];
    assert!(
        super::super::path_instantiation::aggregate_arguments::reference_leaves(
            &program,
            machine,
            local.initial_value,
            local.type_reference,
            "",
            &symbols,
            &mut FrameInference::for_state(make_state.symbol),
        )
        .is_none(),
        "cached effects cannot supply an active producer's result origins"
    );
}

#[test]
fn returned_carrier_does_not_export_a_replaced_parameter_origin() {
    // The explicit borrow replaces the reference; a bare primitive reference
    // RHS would instead copy the referred value through the destination.
    let program = crate::front_end::typed_program(
        "data Carrier { destination: &mut u64; }
         machine initialize(destination: &mut u64) { destination = 7; }
         machine make(destination: &mut u64, replacement: &mut u64) -> Carrier {
             initialize(&mut destination);
             destination = &mut replacement;
             Carrier { destination: destination }
         }
         data Root { scratch: u64; other: u64; }
         machine Root::write(&mut self) {
             let carrier: Carrier = make(&mut self.scratch, &mut self.other);
             carrier.destination = 11;
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::write")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    if let Some(paths) = frames[0].complete_paths() {
        assert!(paths.contains(&"self.other".to_owned()), "{paths:?}");
    }
}

#[test]
fn scalar_method_preserves_carrier_reference_origins() {
    let program = crate::front_end::typed_program(
        "data Carrier { destination: &mut u64; count: u64; }
         machine Carrier::set_count(&mut self) { self.count = 7; }
         data Root { scratch: u64; }
         machine Root::write(&mut self) {
             let mut carrier: Carrier = Carrier { destination: &mut self.scratch, count: 0 };
             carrier.set_count();
             carrier.destination = 11;
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::write")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[0].complete_paths(),
        Some(["self.scratch".to_owned()].as_slice())
    );
}

#[test]
fn carrier_method_reference_replacement_keeps_origins_opaque() {
    let program = crate::front_end::typed_program(
        "data Carrier { destination: &mut u64; count: u64; }
         machine Carrier::replace(&mut self, replacement: &mut u64) {
             self.destination = replacement;
         }
         data Root { scratch: u64; other: u64; }
         machine Root::write(&mut self) {
             let mut carrier: Carrier = Carrier { destination: &mut self.scratch, count: 0 };
             carrier.replace(&mut self.other);
             carrier.destination = 11;
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::write")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert!(
        frames[0].complete_paths().is_none(),
        "replaced slot cannot retain scratch origin"
    );
}

#[test]
fn signature_only_carrier_method_does_not_prove_binding_preservation() {
    let program = crate::front_end::typed_program(
        "pub data Carrier { destination: &mut u64; count: u64; }
         boundary trait Host {}
         boundary machine Carrier::replace(&mut self) reaches Host;
         data Root { scratch: u64; }
         machine Root::write(&mut self) reaches Host {
             let mut carrier: Carrier = Carrier { destination: &mut self.scratch, count: 0 };
             carrier.replace();
             carrier.destination = 11;
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::write")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert!(
        frames[0].complete_paths().is_none(),
        "unknown method may replace the stored reference"
    );
}

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
            assert_eq!(
                stored[0].references[0]
                    .origin
                    .as_ref()
                    .expect("known source")
                    .path,
                "shared.value"
            );
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
        for candidate in program.machine_states(machine) {
            assert!(
                symbols
                    .caller_sites
                    .completed_state_writes(candidate.symbol)
                    .is_none(),
                "prefixes are not complete summaries"
            );
        }
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

    let written =
        summarize_complete_state_written_paths(&program, machine, state, &symbols, &mut inference)
            .unwrap();
    assert_eq!(written, ["value"]);

    for include_shared in [false, true] {
        let (prefix, boundaries) = walk_state_write_prefix_collected(
            &program,
            machine,
            state,
            &symbols,
            &mut inference,
            include_shared,
            &|_| true,
        )
        .unwrap();
        assert_eq!(prefix.written, ["value"]);
        assert_eq!(boundaries.len(), 2);
    }
}

#[test]
fn selected_boundaries_preserve_alias_versions_and_complete_effects() {
    let program = crate::front_end::typed_program(
        "data Pair { left: u64; right: u64; }
         data Carrier { value: &mut u64; }
         machine inspect(pair: &mut Pair) {
             let mut selected: &mut u64 = &mut pair.left;
             let carrier: Carrier = Carrier { value: selected };
             selected = &mut pair.right;
             carrier.value = 7;
             selected = 9;
         }",
    );
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    let machine = symbols.machine("inspect").unwrap();
    let state = &program.machine_states(machine)[0];
    for include_shared in [false, true] {
        let collect = |requested: &dyn Fn(usize) -> bool| {
            walk_state_write_prefix_collected(
                &program,
                machine,
                state,
                &symbols,
                &mut FrameInference::default(),
                include_shared,
                requested,
            )
            .expect("complete transfer")
        };
        let (complete, all) = collect(&|_| true);
        let (selected, boundaries) = collect(&|position| position == 2 || position == 4);
        assert_eq!(all.iter().flatten().count(), 5);
        assert_eq!(boundaries.iter().flatten().count(), 2);
        assert_eq!(boundaries.len(), all.len());
        assert_eq!(selected.written, complete.written);
        assert_eq!(selected.written, ["pair.left", "pair.right"]);
        for position in [2, 4] {
            // Compare every evidence field, including stored case/move rows,
            // without imposing equality on production-only working records.
            assert_eq!(
                format!("{:?}", boundaries[position]),
                format!("{:?}", all[position])
            );
            let boundary = boundaries[position].as_ref().unwrap();
            let alias = boundary
                .aliases
                .iter()
                .find(|(name, _)| name == "selected")
                .unwrap();
            assert_eq!(
                alias.1.path,
                if position == 2 {
                    "pair.left"
                } else {
                    "pair.right"
                }
            );
            assert!(!boundary.stored.is_empty());
        }
        let (without_snapshots, boundaries) = collect(&|_| false);
        assert!(boundaries.iter().all(Option::is_none));
        assert_eq!(without_snapshots.written, complete.written);
    }
}

#[test]
fn selected_boundaries_do_not_hide_later_unknown_effects() {
    let program = crate::front_end::typed_program(
        "pub data Carrier { destination: &mut u64; }
         boundary trait Host {}
         boundary machine Carrier::replace(&mut self) reaches Host;
         machine inspect(value: &mut u64) reaches Host {
             let mut carrier: Carrier = Carrier { destination: value };
             carrier.replace();
             carrier.destination = 11;
         }",
    );
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    let machine = symbols.machine("inspect").unwrap();
    let state = &program.machine_states(machine)[0];
    for include_shared in [false, true] {
        let result = walk_state_write_prefix_collected(
            &program,
            machine,
            state,
            &symbols,
            &mut FrameInference::default(),
            include_shared,
            &|position| position == 1,
        );
        assert!(
            result.is_none(),
            "a selected prefix is not a complete result"
        );
    }
}

#[test]
fn result_boundaries_compose_named_and_value_carrier_returns() {
    let program = crate::front_end::typed_program(
        "data Carrier { destination: &mut u64; }
         machine make(first: &mut u64, second: &mut u64, choose: bool) -> Carrier {
             let left: Carrier = Carrier { destination: first };
             transition choose {
                 true -> (left)
                 _ -> finish(second)
             }
             state finish(other: &mut u64) -> Carrier {
                 let right: Carrier = Carrier { destination: other };
                 right
             }
         }
         machine write(first: &mut u64, second: &mut u64, choose: bool) {
             let carrier: Carrier = make(first, second, choose);
             carrier.destination = 11;
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "write")
        .unwrap();
    let frames = crate::CallFrameResolver::new(&program)
        .expect("resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[0].complete_paths(),
        Some(["$P0".to_owned(), "$P1".to_owned()].as_slice())
    );
}

/// A completed owned result does not expose the construction writes to its
/// caller. A payload-free case field must not make those private writes opaque.
#[test]
fn owned_local_result_initialization_has_a_complete_private_frame() {
    for (kind_type, kind_value) in [("u32 in Wrapping", "0"), ("EnemyKind", "EnemyKind::Bat")] {
        let source = format!(
            "domain [u8; 24]::Utf8 requires valid_utf8(self);
             data EnemyKind [copy] {{ case Bat; case Spider; }}
             data Enemy [copy] {{
                 kind: {kind_type};
                 name: [u8; 24] in Utf8;
                 health: i32 in Wrapping;
             }}
             data Builder {{}}
             machine Builder::empty_enemy(&mut self) -> Enemy {{
                 let enemy: Enemy;
                 enemy.kind = {kind_value};
                 enemy.name = \"\";
                 enemy.health = 1;
                 transition {{ _ -> enemy }}
             }}"
        );
        let program = crate::front_end::typed_program(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Builder::empty_enemy")
            .expect("owned-result producer");
        let frames = crate::CallFrameResolver::new(&program)
            .expect("frame resolver")
            .inferred_machine_state_write_frames(machine);
        assert_eq!(frames.len(), 1);
        if kind_type == "EnemyKind" {
            let state = &program.machine_states(machine)[0];
            let statements = program.statement_table.statements(state.statement_nodes);
            let typed_trees::statement::StatementNode::LocalData(local) = &statements[0] else {
                panic!("owned local declaration");
            };
            let typed_trees::statement::StatementNode::Assignment(assignment) = &statements[1]
            else {
                panic!("case initialization");
            };
            let constructor = crate::scalar_case_constructor(&program, assignment.value)
                .expect("exact named case constructor");
            let mut diagnostics = Vec::new();
            let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
            assert!(diagnostics.is_empty());
            let origins = |reference, suffix| {
                super::super::path_instantiation::aggregate_arguments::reference_leaves(
                    &program,
                    machine,
                    assignment.value,
                    reference,
                    suffix,
                    &symbols,
                    &mut FrameInference::default(),
                )
            };
            let selected = origins(constructor.type_reference, "").expect("constructed case");
            assert!(selected.references.is_empty());
            assert!(selected.moves.is_empty());
            assert_eq!(
                selected.cases,
                vec![vec![facts::PlaceSegment::Case {
                    variant: constructor.case,
                }]]
            );
            assert!(origins(local.type_reference, "").is_none(), "wrong owner");
            assert!(
                origins(constructor.type_reference, ".missing").is_none(),
                "a payload-free case supplies no projected payload"
            );
            let prefixes = collect_state_write_prefixes(&program, machine, state, &symbols, false)
                .expect("private construction prefixes");
            let before = prefixes[1].as_ref().expect("declared local frontier");
            let fresh = before
                .stored
                .iter()
                .find(|row| row.local_symbol == local.symbol)
                .expect("fresh local case inventory");
            let incoming = super::super::stored_origins::declared_origins(
                &program,
                local.symbol,
                local.name.as_str(),
                local.type_reference,
            )
            .expect("input case inventory");
            assert_eq!(
                fresh.cases, incoming.cases,
                "preserve the symbolic case roster"
            );
            assert!(fresh.references.is_empty());
            assert!(fresh.moves.is_empty(), "fresh storage transports no input");
            assert!(
                !incoming.moves.is_empty(),
                "input identity remains indivisible"
            );
            for (position, prefix) in prefixes.iter().enumerate() {
                assert!(
                    prefix.is_some(),
                    "private construction lost its frame before statement {position}: {:?}",
                    statements[position]
                );
            }
            let moved_source = source
                .replace(
                    "(&mut self) -> Enemy",
                    "(&mut self, incoming: Enemy) -> Enemy",
                )
                .replace("let enemy: Enemy;", "let enemy: Enemy = incoming;");
            let moved_program = crate::front_end::typed_program(&moved_source);
            let moved_machine = moved_program
                .machines()
                .iter()
                .find(|candidate| candidate.name.as_str() == "Builder::empty_enemy")
                .unwrap();
            let moved_frames = crate::CallFrameResolver::new(&moved_program)
                .unwrap()
                .inferred_machine_state_write_frames(moved_machine);
            assert!(
                moved_frames[0].complete_paths().is_none(),
                "an actual moved subtree cannot keep stale origins after an interior replacement"
            );
        }
        assert_eq!(
            frames[0].complete_paths(),
            Some([].as_slice()),
            "{kind_type}: private local construction must retain a complete empty caller frame; {source}"
        );
    }
}
