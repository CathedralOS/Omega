use super::*;

#[test]
fn ordinary_empty_transition_arms_owe_machine_postconditions() {
    for body in [
        "transition { _ -> {} }",
        "transition self.flag { true -> {} false -> {} }",
    ] {
        let source = format!(
            "data Main {{ flag: bool; }} machine Main::run(&mut self) ensures false {{ {body} }}"
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source))
            .expect_err("ordinary Unit exit owes ensures");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn crash_and_named_dispatch_do_not_create_fallthrough_exits() {
    let checked = lower_typed_trees(parse_typed_trees(
        r#"
        machine fail()
        crashes Abort
        ensures false
        { crash Abort; }
        "#,
    ))
    .expect("crash does not return ordinarily");
    assert_eq!(checked.facts.proof.contract_exits.len(), 0);

    let checked = lower_typed_trees(parse_typed_trees(
        r#"
        data Main {}
        machine Main::run(&mut self)
        ensures true
        {
            transition { _ -> done() }
            state done(&mut self) {}
        }
        "#,
    ))
    .expect("the final named state, not its incoming jump, returns");
    let exits = checked
        .facts
        .proof
        .contract_exits
        .iter()
        .map(|(_, exit)| exit)
        .collect::<Vec<_>>();
    assert_eq!(exits.len(), 1);
    assert!(!exits[0].transition_target.is_valid());
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("machine");
    let done = checked
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "done")
        .expect("done");
    assert_eq!(exits[0].state_symbol, done.symbol);
}

#[test]
fn returning_arm_guards_prove_only_their_own_truth_value() {
    for (ensures, accepted) in [("self.flag", true), ("self.flag == false", false)] {
        let source = format!(
            r#"
            data Main {{ flag: bool; }}
            machine Main::run(&mut self) ensures {ensures}
            {{
                transition self.flag {{ true -> {{}} false -> fail() }}
                state fail(&mut self) {{ crash Abort; }}
            }}
        "#
        );
        let result = lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{ensures}: {result:#?}");
    }
    let source = r#"
        data Main { flag: bool; }
        machine Main::run(&mut self) ensures self.flag == false {
            transition self.flag { true -> fail() false -> {} }
            state fail(&mut self) { crash Abort; }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("false arm retains negative guard evidence");
}

#[test]
fn branch_call_postconditions_do_not_leak_to_sibling_returns() {
    let source = r#"
        data Main { flag: bool; value: u64; }
        boundary machine establish(value: &mut u64) -> u64 ensures value == 1;
        boundary machine unknown(value: &mut u64) -> u64;
        machine Main::run(&mut self) -> u64 ensures self.value == 1 {
            transition self.flag {
                true -> (establish(&mut self.value))
                false -> (unknown(&mut self.value))
            }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a sibling guarantee cannot prove an unknown return arm");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
}

#[test]
fn target_writes_invalidate_all_copies_of_multi_place_guard_evidence() {
    let source = r#"
        data Main { left: u64; right: u64; }
        machine change(value: &mut u64) -> u64 { value = 7; 0 }
        machine Main::run(&mut self) -> u64 ensures self.left == self.right {
            transition self.left == self.right {
                true -> (change(&mut self.left))
                false -> fail()
            }
            state fail(&mut self) { crash Abort; }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("one changed guard input invalidates the complete condition");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
}

#[test]
fn target_selector_writes_invalidate_guarded_member_reads() {
    let source = r#"
        data Cell { value: u64; }
        data Main { cells: [Cell; 2]; index: u64 [0..=1]; }
        machine change(value: &mut u64 [0..=1]) -> u64 { value = 1; 0 }
        machine Main::run(&mut self) -> u64 ensures self.cells[self.index].value == 0 {
            transition self.cells[self.index].value == 0 {
                true -> (change(&mut self.index))
                false -> fail()
            }
            state fail(&mut self) { crash Abort; }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("selector writes invalidate the selected member condition");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
}

#[test]
fn renamed_output_reference_establishes_the_entry_postcondition() {
    for route in [
        "transition { _ -> write(out_line) } state write(destination: &mut [u8; 4]) { destination = \"ok\"; }",
        "transition { _ -> forward(out_line) } state forward(buffer: &mut [u8; 4]) { transition { _ -> write(buffer) } } state write(destination: &mut [u8; 4]) { destination = \"ok\"; }",
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine fill(out_line: &mut [u8; 4]) ensures out_line in Utf8 {{ {route} }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{route}: {diagnostics:#?}"));
    }
}

#[test]
fn named_exits_cannot_reuse_stale_entry_requires() {
    for route in [
        "out_line = [255, 0, 0, 0]; transition { _ -> done(out_line) } state done(destination: &mut [u8; 4]) {}",
        "transition { _ -> done(out_line) } state done(destination: &mut [u8; 4]) { destination = [255, 0, 0, 0]; }",
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine fill(out_line: &mut [u8; 4]) requires out_line in Utf8;
            ensures out_line in Utf8 {{ {route} }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source))
            .expect_err("renaming cannot refresh a stale entry assumption");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn loop_carried_reference_origins_retain_the_entry_output() {
    for repeat_target in [
        "visit(destination, repeat)",
        "again(destination, repeat)",
        "self",
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine fill(out_line: &mut [u8; 4], repeat: bool)
            ensures out_line in Utf8 {{
                transition {{ _ -> visit(out_line, repeat) }}
                state visit(destination: &mut [u8; 4], repeat: bool) {{
                    transition repeat {{ true -> {repeat_target} false -> write(destination) }}
                }}
                state again(buffer: &mut [u8; 4], repeat: bool) {{
                    transition {{ _ -> visit(buffer, repeat) }}
                }}
                state write(result: &mut [u8; 4]) {{ result = "ok"; }}
            }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{repeat_target}: {diagnostics:#?}"));
    }
}

#[test]
fn loop_carried_reference_origins_do_not_choose_between_swapped_inputs() {
    for repeat_target in [
        "visit(spare, destination, repeat)",
        "again(spare, destination, repeat)",
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine fill(out_line: &mut [u8; 4], other: &mut [u8; 4], repeat: bool)
            ensures out_line in Utf8 {{
                transition {{ _ -> visit(out_line, other, repeat) }}
                state visit(destination: &mut [u8; 4], spare: &mut [u8; 4], repeat: bool) {{
                    transition repeat {{ true -> {repeat_target} false -> write(destination) }}
                }}
                state again(first: &mut [u8; 4], second: &mut [u8; 4], repeat: bool) {{
                    transition {{ _ -> visit(first, second, repeat) }}
                }}
                state write(result: &mut [u8; 4]) {{ result = "ok"; }}
            }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source))
            .expect_err("a loop can exchange the two independently borrowed outputs");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("no exact incoming reference origin for out_line")),
            "{repeat_target}: {diagnostics:#?}"
        );
    }
}

#[test]
fn loop_exits_do_not_refresh_authored_entry_assumptions() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4], repeat: bool)
        requires out_line in Utf8;
        ensures out_line in Utf8 {
            out_line = [255, 0, 0, 0];
            transition { _ -> visit(out_line, repeat) }
            state visit(destination: &mut [u8; 4], repeat: bool) {
                transition repeat { true -> visit(destination, repeat) false -> {} }
            }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("identity preservation is not preservation of an earlier domain fact");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no exact incoming reference origin")),
        "{diagnostics:#?}"
    );
}

#[test]
fn loop_reference_origins_retain_unknown_incoming_alternatives() {
    for (declaration, argument) in [
        ("let alias: &mut [u8; 4] = destination;", "alias"),
        ("let mut local: [u8; 4] = [0, 0, 0, 0];", "&mut local"),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine fill(out_line: &mut [u8; 4], repeat: bool)
            ensures out_line in Utf8 {{
                transition {{ _ -> visit(out_line, repeat) }}
                state visit(destination: &mut [u8; 4], repeat: bool) {{
                    transition repeat {{ true -> again(destination, repeat) false -> write(destination) }}
                }}
                state again(destination: &mut [u8; 4], repeat: bool) {{
                    {declaration}
                    transition {{ _ -> visit({argument}, repeat) }}
                }}
                state write(result: &mut [u8; 4]) {{ result = "ok"; }}
            }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source))
            .expect_err("the direct entry edge cannot erase an unknown loop-carried origin");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("no exact incoming reference origin for out_line")),
            "{declaration}: {diagnostics:#?}"
        );
    }
}

#[test]
fn unreachable_predecessors_do_not_contaminate_reference_origins() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4]) ensures out_line in Utf8 {
            transition { _ -> write(out_line) }
            state detached(unrelated: &mut [u8; 4], repeat: bool) {
                transition repeat { true -> detached(unrelated, repeat) false -> write(unrelated) }
            }
            state write(destination: &mut [u8; 4]) { destination = "ok"; }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("an entry-unreachable component supplies no runtime incoming reference");
}

#[test]
fn named_exit_reference_origin_requires_a_stable_final_binding() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4], other: &mut [u8; 4])
        ensures out_line in Utf8 {
            transition { _ -> write(out_line, other) }
            state write(mut destination: &mut [u8; 4], replacement: &mut [u8; 4]) {
                destination = replacement;
                destination = "ok";
            }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the final write belongs to the replacement, not the old entry output");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no exact incoming reference origin for out_line")),
        "{diagnostics:#?}"
    );
}

#[test]
fn entry_exit_reference_origin_requires_a_stable_final_binding() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(mut output: &mut [u8; 4], replacement: &mut [u8; 4])
        ensures output in Utf8 {
            output = replacement;
            output = "okay";
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("rebinding the entry parameter cannot qualify the original caller referent");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
}

#[test]
fn ambiguous_named_origins_do_not_select_one_incoming_reference() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4], other: &mut [u8; 4], flag: bool)
        ensures out_line in Utf8 {
            transition flag { true -> write(out_line) false -> write(other) }
            state write(destination: &mut [u8; 4]) { destination = "ok"; }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a destination that may name another input cannot prove this output");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no exact incoming reference origin for out_line")),
        "{diagnostics:#?}"
    );
}

#[test]
fn named_origin_mapping_does_not_use_equal_names_or_transformed_values() {
    for source in [
        r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4], other: &mut [u8; 4])
        ensures out_line in Utf8 {
            transition { _ -> write(other) }
            state write(out_line: &mut [u8; 4]) { out_line = "ok"; }
        }
        "#,
        r#"
        machine fill(value: u64)
        ensures value == 0 {
            transition { _ -> done(0) }
            state done(value: u64) requires value == 0; {}
        }
        "#,
    ] {
        let diagnostics = lower_typed_trees(parse_typed_trees(source))
            .expect_err("a fresh target binding is not the entry value");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn named_reference_origin_requires_a_stable_input_binding() {
    for (prefix, stable) in [
        ("", true),
        ("out_line = other;", false),
        ("replace(&mut out_line, other);", false),
        (
            "let slot: &mut [u8; 4] = &mut out_line; replace(slot, other);",
            false,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            boundary machine replace(slot: &mut [u8; 4], replacement: &mut [u8; 4]);
            machine fill(mut out_line: &mut [u8; 4], other: &mut [u8; 4])
            ensures out_line in Utf8 {{
                {prefix}
                transition {{ _ -> write(out_line) }}
                state write(destination: &mut [u8; 4]) {{ destination = "ok"; }}
            }}
        "#
        );
        let program = parse_typed_trees(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "fill")
            .expect("fill");
        let state = &program.machine_states(machine)[0];
        let parameter = program.state_parameters(state)[0].symbol;
        assert_eq!(
            psi_validation::state_reference_parameter_binding_is_stable(
                &program, machine, state, parameter
            ),
            stable,
            "{prefix}"
        );
        let result = lower_typed_trees(program);
        assert_eq!(result.is_ok(), stable, "{prefix}: {result:#?}");
    }
}

#[test]
fn named_reference_origin_requires_consistent_whole_name_symbols() {
    use psi_typed_trees::expression::ExpressionNode;
    use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
    let original = parse_typed_trees(
        r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4], other: &mut [u8; 4])
        ensures out_line in Utf8 {
            transition { _ -> write(out_line) }
            state write(destination: &mut [u8; 4]) { destination = "ok"; }
        }
    "#,
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "fill")
        .expect("fill");
    let state = &original.machine_states(machine)[0];
    let StatementNode::Transition(transition) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("transition");
    };
    let TransitionTargetNode::Named { arguments, .. } = original
        .statement_table
        .transition_target(transition.target)
    else {
        panic!("named target");
    };
    let expression = original.statement_table.expression_handles(*arguments)[0];
    let exact = original.state_parameters(state)[0].symbol;
    let foreign = original.state_parameters(state)[1].symbol;
    for (name, symbol, accepted) in [
        ("exact", exact, true),
        ("foreign_final", foreign, false),
        ("missing_final", psi_symbols::SymbolHandle::invalid(), false),
        (
            "stale_final",
            psi_symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
            panic!("argument");
        };
        path.symbol = symbol;
        let result = lower_typed_trees(program);
        assert_eq!(result.is_ok(), accepted, "{name}: {result:#?}");
    }
}

#[test]
fn named_state_self_receives_enforced_field_domains_not_stale_requires() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Main { line: [u8; 4] in Utf8; }
        machine consume(line: [u8; 4] in Utf8) {}
        machine Main::run(&mut self) {
            transition { _ -> read() }
            state read(&mut self) { consume(self.line); }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("the explicit self parameter retains enforced field qualifications");

    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Main { line: [u8; 4] in Utf8; }
        machine consume(line: [u8; 4] in Utf8) {}
        machine Main::run(&mut self) {
            self.line[0] = 255;
            transition { _ -> read() }
            state read(&mut self) { consume(self.line); }
        }
    "#;
    assert!(
        lower_typed_trees(parse_typed_trees(source)).is_err(),
        "an invalid field write cannot re-establish its domain by jumping"
    );
}

#[test]
fn rebased_contexts_grow_linearly_and_stay_scoped_to_their_state() {
    let mut sizes = Vec::new();
    for state_count in [4, 12] {
        let mut states = String::new();
        for index in 0..state_count {
            let body = if index + 1 == state_count {
                String::new()
            } else {
                format!(
                    "transition self.flag {{ true -> {{}} false -> stage{}() }}",
                    index + 1
                )
            };
            states.push_str(&format!("state stage{index}(&self) {{ {body} }}\n"));
        }
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            data Main {{ line: [u8; 4] in Utf8; flag: bool; }}
            machine Main::run(&self) ensures self.line in Utf8 {{
                transition {{ _ -> stage0() }}
                {states}
            }}
        "#
        );
        let checked = lower_typed_trees(parse_typed_trees(&source))
            .expect("state-local declared field facts preserve the output qualification");
        let semantic = &checked.facts.semantic;
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("machine");
        for (_, state) in checked
            .facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == machine.symbol)
        {
            for reference in checked
                .facts
                .flow
                .contexts
                .semantic_context_refs
                .span_or_empty(state.entry_semantic_contexts)
            {
                let point = semantic.contexts.get(reference.context).point;
                assert!(
                    matches!(
                        point,
                        psi_facts::ProgramPoint::Global | psi_facts::ProgramPoint::Machine { .. }
                    ) || matches!(point, psi_facts::ProgramPoint::State { state_symbol, .. }
                        if state_symbol == state.state_symbol),
                    "an entry context belongs to a sibling or exit: {point:?}"
                );
            }
        }
        sizes.push((
            semantic.contexts.len(),
            semantic
                .context_handles_at_point(psi_facts::ProgramPoint::Global)
                .count(),
            semantic
                .context_handles_at_point(psi_facts::ProgramPoint::Machine {
                    machine_symbol: machine.symbol,
                })
                .count(),
        ));
    }
    assert!(
        sizes[1].0 <= sizes[0].0 * 4,
        "context growth must stay linear: {sizes:?}"
    );
    assert_eq!(sizes[0].1, sizes[1].1, "global contexts must be shared");
    assert_eq!(
        sizes[0].2, sizes[1].2,
        "rebasing must not republish machine contexts"
    );
}

#[test]
fn rebased_exit_requirements_do_not_become_sibling_entry_facts() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Main { line: [u8; 4]; flag: bool; }
        machine Main::run(&mut self) ensures self.line in Utf8 {
            transition self.flag { true -> good() false -> bad() }
            state good(&mut self) { self.line = "ok"; }
            state bad(&mut self) {}
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the good exit's guarantee is not a premise at the bad state");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
    let repaired = source.replace(
        "state bad(&mut self) {}",
        "state bad(&mut self) { self.line = \"ok\"; }",
    );
    lower_typed_trees(parse_typed_trees(&repaired))
        .expect("both branches establish their own output instead of sharing a guarantee");
}
