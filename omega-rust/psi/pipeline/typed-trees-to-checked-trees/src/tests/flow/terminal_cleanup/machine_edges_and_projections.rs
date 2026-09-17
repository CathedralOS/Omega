use super::{checked, machine_and_entry_state, typed_program};

#[test]
fn named_machine_back_edge_retains_entry_state_cleanup_and_shared_unit_plan() {
    let checked = checked(
        r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8]) reaches Output {
            transition bytes.len > 0 {
                true -> emit(bytes[0] as i32, bytes[1..])
                false -> done()
            }
            state emit(head: i32, bytes: &[u8]) {
                Output::write(bytes, head);
                transition { _ -> relay(bytes) }
            }
            state done() {}
        }
        "#,
    );
    let (machine, entry) = machine_and_entry_state(&checked, "relay");
    assert_ne!(
        machine, entry,
        "source machine and entry state have distinct symbols"
    );
    let declaration = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .unwrap();
    let emit = checked
        .machine_states(declaration)
        .iter()
        .find(|state| state.name.as_str() == "emit")
        .unwrap();
    let cleanup = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_edge(machine, emit.symbol, 1)
        .expect("the named machine back-edge retains its own cleanup row after the call");
    assert_eq!(cleanup.target_state, entry);
    assert!(
        cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("the cleanup row closes the reentered shared Unit graph");
    let emit_plan = plan
        .states
        .iter()
        .find(|state| state.state == emit.symbol)
        .unwrap();
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &emit_plan.terminator
    else {
        panic!("the emit state jumps back to entry");
    };
    assert_eq!(successor.target_state, entry);
    assert_eq!(
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            &checked.facts
        ),
        checked.facts.flow.terminal_structural_control_cleanups,
    );
}

#[test]
fn structural_conditional_edges_retain_independent_affine_parameter_cleanup() {
    let checked = checked(
        r#"
        data Token { value: i32; }

        machine route(first: Token, second: Token, choose_first: bool) -> i32
        {
            transition choose_first {
                true -> keep_first(first)
                _ -> keep_second(second)
            }

            state keep_first(first: Token) -> i32 { 1 }
            state keep_second(second: Token) -> i32 { 2 }
        }
        "#,
    );
    let (machine, entry) = machine_and_entry_state(&checked, "route");
    let plan = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_state(machine, entry)
        .expect("the checked structural conditional has an edge cleanup plan");
    assert_eq!(plan.edges.len(), 2);
    assert_eq!(plan.edges[0].statement_ordinal, 0);
    assert_eq!(
        plan.edges[0].trivial_affine_discard_parameter_positions,
        [1]
    );
    assert_eq!(plan.edges[1].statement_ordinal, 1);
    assert_eq!(
        plan.edges[1].trivial_affine_discard_parameter_positions,
        [0]
    );
    let rebuilt =
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            &checked.facts,
        );
    assert_eq!(
        rebuilt, checked.facts.flow.terminal_structural_control_cleanups,
        "the checked edge plan is reconstructed from typed ownership evidence"
    );
    let mut missing_evidence = checked.facts.clone();
    missing_evidence.flow.ownership.permissions = Default::default();
    assert!(
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            &missing_evidence,
        )
        .for_state(machine, entry)
        .is_none(),
        "missing state-exit evidence must not become an empty cleanup plan"
    );
}

#[test]
fn structural_control_only_fences_edges_that_discard_nominal_cleanup() {
    let checked = checked(
        r#"
        data Nominal {}
        machine Nominal::drop(&mut self) {}
        data Wrapper { value: Nominal; }
        data Plain { value: u64; }

        machine route(value: Plain) {
            transition { _ -> keep(value) }
            state keep(value: Plain) {}
        }

        machine lose(value: Wrapper) {
            transition { _ -> done() }
            state done() {}
        }
        "#,
    );

    let (route, route_entry) = machine_and_entry_state(&checked, "route");
    let route_plan = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_state(route, route_entry)
        .expect("plain whole-value transfer remains in the structural-control slice");
    assert!(
        route_plan.edges[0]
            .trivial_affine_discard_parameter_positions
            .is_empty()
    );

    let (lose, lose_entry) = machine_and_entry_state(&checked, "lose");
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_state(lose, lose_entry)
            .is_none(),
        "an edge requiring nominal cleanup cannot publish a no-code discard row"
    );
}

#[test]
fn structural_jump_retains_reverse_order_cleanup_after_transfer() {
    let checked = checked(
        r#"
        data Token { value: i32; }

        machine route(first: Token, second: Token, third: Token) -> i32
        {
            transition { _ -> next(second) }
            state next(second: Token) -> i32 { 0 }
        }
        "#,
    );
    let (machine, entry) = machine_and_entry_state(&checked, "route");
    let plan = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_state(machine, entry)
        .expect("the checked structural jump has an edge cleanup plan");
    let [edge] = plan.edges.as_slice() else {
        panic!("the jump has one edge")
    };
    assert_eq!(edge.statement_ordinal, 0);
    assert_eq!(edge.trivial_affine_discard_parameter_positions, [2, 0]);
}

#[test]
fn affine_locals_fail_closed_in_the_whole_parameter_edge_slice() {
    // Since 8c3d008f6c a plain affine record local is a state-exit result
    // local: its disposal is partitioned out of the whole-parameter roster and
    // the graph producer rejoins it, so the parameter-only edge plan publishes
    // beside the retained state-exit drop. A local whose disposal needs code
    // still withholds the whole state plan rather than publishing a partial
    // parameter-only slice.
    for (declarations, local, published) in [
        ("data Token { value: i32; }", "Token { value: 1 }", true),
        (
            "data Token { value: i32; } data Nominal {} machine Nominal::drop(&mut self) {}",
            "Nominal {}",
            false,
        ),
    ] {
        let local_type = local.split(' ').next().unwrap();
        let checked = checked(&format!(
            r#"
            {declarations}

            machine route(input: Token) -> i32
            {{
                let local: {local_type} = {local};
                transition {{ _ -> next(input) }}
                state next(input: Token) -> i32 {{ 0 }}
            }}
            "#
        ));
        let (machine, entry) = machine_and_entry_state(&checked, "route");
        let plan = checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_state(machine, entry);
        if !published {
            assert!(
                plan.is_none(),
                "`{local_type}` local cleanup must not publish a partial parameter-only plan"
            );
            continue;
        }
        let plan = plan.expect("plain affine local partitions out of the parameter edge slice");
        let [edge] = plan.edges.as_slice() else {
            panic!("one ordinary successor edge: {:?}", plan.edges);
        };
        assert_eq!(edge.statement_ordinal, 1);
        assert!(
            edge.trivial_affine_discard_parameter_positions.is_empty(),
            "the transferred parameter leaves no whole-parameter discard: {:?}",
            edge.trivial_affine_discard_parameter_positions
        );
        let declaration = checked
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == machine)
            .unwrap();
        let state = &checked.machine_states(declaration)[0];
        let typed_trees::statement::StatementNode::LocalData(local) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("the local declaration opens the entry state");
        };
        assert!(
            checked
                .facts
                .flow
                .ownership
                .permissions
                .iter()
                .any(|(_, event)| {
                    event.machine_symbol == machine
                        && event.state_symbol == entry
                        && event.source == language_semantics::PermissionEventSource::StateExit
                        && event.kind == language_semantics::PermissionEventKind::AffineDrop
                        && event.root == ::facts::PlaceRoot::Symbol(local.symbol)
                }),
            "the local's disposal stays a retained state-exit drop, not a parameter discard"
        );
    }
}

#[test]
fn partial_affine_parameter_moves_fail_closed() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Pair { left: Token; right: Token; }

        machine route(pair: Pair) -> i32
        {
            transition { _ -> next(pair.left) }
            state next(token: Token) -> i32 { 0 }
        }
        "#,
    );
    let (machine, entry) = machine_and_entry_state(&checked, "route");
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_state(machine, entry)
            .is_none(),
        "a projected move needs a partial-value cleanup plan, not a whole-parameter discard"
    );
}

#[test]
fn attached_unit_direct_record_projection_retains_transfer_and_maximal_sibling() {
    for (moved, residual) in [("left", "right"), ("right", "left")] {
        let checked = checked(&format!(
            r#"
            data Token {{ value: i32; }}
            data Pair {{ left: Token; right: Token; }}
            data Root {{}}

            machine Root::route(pair: Pair)
            {{
                transition {{ _ -> next(pair.{moved}) }}
                state next(token: Token) {{}}
            }}
            "#,
        ));
        let (machine, entry) = machine_and_entry_state(&checked, "route");
        let edge = checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_projected_edge(machine, entry, 0)
            .expect("the bounded direct-field jump should retain checked cleanup");
        assert_eq!(edge.machine, machine);
        assert_eq!(edge.state, entry);
        assert_eq!(edge.statement_ordinal, 0);
        assert_eq!(edge.transfer.source_parameter_position, 0);
        assert_eq!(edge.transfer.target_parameter_position, 0);
        assert_eq!(edge.transfer.path.len(), 1);
        assert!(matches!(
            &edge.transfer.path[0],
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity)
                if identity.ends_with(moved)
        ));
        let [sibling] = edge.residual_affine_discards.as_slice() else {
            panic!("exactly one maximal sibling residual should remain")
        };
        assert_eq!(
            sibling.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: 0
            }
        );
        assert_eq!(sibling.path.len(), 1);
        assert!(matches!(
            &sibling.path[0],
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity)
                if identity.ends_with(residual)
        ));
        assert_eq!(edge.transfer.type_identity, sibling.type_identity);
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_control_cleanups
                .for_edge(machine, entry, 0)
                .is_none(),
            "whole-root consumers must fail closed on a projected edge"
        );
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_unit_controls
                .for_machine(machine)
                .is_none(),
            "Terminal structural control deliberately has no path-segment jump carrier"
        );

        let rebuilt =
            crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
                &checked.typed,
                &checked.facts,
            );
        assert_eq!(
            rebuilt,
            checked.facts.flow.terminal_structural_control_cleanups
        );
        let mut drifted = rebuilt.clone();
        let drifted_edge = drifted
            .projected_edges
            .iter_mut()
            .find(|candidate| candidate.machine == machine && candidate.state == entry)
            .expect("projected row to mutate");
        drifted_edge.transfer.type_identity.push_str("-drift");
        drifted_edge.residual_affine_discards[0].path = drifted_edge.transfer.path.clone();
        assert_ne!(
            drifted,
            crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
                &checked.typed,
                &checked.facts,
            ),
            "transfer type and residual-path drift must not match reconstruction"
        );

        let mut missing_exit = checked.facts.clone();
        missing_exit.flow.ownership.permissions = Default::default();
        assert!(
            crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
                &checked.typed,
                &missing_exit,
            )
            .for_projected_edge(machine, entry, 0)
            .is_none(),
            "missing authoritative affine-drop evidence must reject"
        );

        let mut overlapping = rebuilt.clone();
        overlapping
            .states
            .push(checked_trees::CheckedStructuralControlStateCleanupPlan {
                machine,
                state: entry,
                edges: vec![checked_trees::CheckedStructuralControlEdgeCleanupPlan {
                    statement_ordinal: 0,
                    target_state: edge.target_state,
                    trivial_affine_discard_parameter_positions: Vec::new(),
                }],
            });
        assert!(
            overlapping.for_edge(machine, entry, 0).is_none(),
            "a forged whole-root overlap cannot hide the projected cleanup row"
        );
    }
}

#[test]
fn projected_transition_cleanup_fences_shapes_outside_the_first_checked_cohort() {
    let sources = [
        (
            "free machine",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            machine route(pair: Pair) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "three-field root",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; middle: Token; right: Token; }
            data Root {}
            machine Root::route(pair: Pair) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "extra affine root",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Root {}
            machine Root::route(pair: Pair, extra: Token) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "non-Unit states",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Root {}
            machine Root::route(pair: Pair) -> i32 {
                transition { _ -> next(pair.left) }
                state next(token: Token) -> i32 { 0 }
            }
            "#,
        ),
        (
            "nested projection",
            r#"
            data Token { value: i32; }
            data Inner { token: Token; spare: Token; }
            data Pair { left: Inner; right: Token; }
            data Root {}
            machine Root::route(pair: Pair) {
                transition { _ -> next(pair.left.token) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "nominal sibling cleanup",
            r#"
            data Token { value: i32; }
            data Resource {}
            machine Resource::drop(&mut self) {}
            data Pair { left: Token; right: Resource; }
            data Root {}
            machine Root::route(pair: Pair) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
    ];
    for (case, source) in sources {
        let checked = checked(source);
        let (machine, entry) = machine_and_entry_state(&checked, "route");
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_control_cleanups
                .for_projected_edge(machine, entry, 0)
                .is_none(),
            "{case} must remain outside the bounded projected cleanup cohort"
        );
    }
}

#[test]
fn projected_transition_shape_helper_rejects_nominal_root_cleanup() {
    let typed = typed_program(
        r#"
        data Token { value: i32; }
        data Pair { left: Token; right: Token; }
        machine Pair::drop(&mut self) {}
        data Root {}
        machine Root::route(pair: Pair) {
            transition { _ -> next(pair.left) }
            state next(token: Token) {}
        }
        "#,
    );
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine");
    let [entry, target] = typed.machine_states(machine) else {
        panic!("route has exactly two states")
    };
    let [source_parameter] = typed.state_parameters(entry) else {
        panic!("entry has one non-self parameter")
    };
    let [target_parameter] = typed.state_parameters(target) else {
        panic!("target has one non-self parameter")
    };
    let pair = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Pair")
        .expect("Pair definition");
    let moved_field = typed
        .data_members(pair)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "left" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("left field");
    assert!(
        crate::execution::exact_two_field_record_projection_for_test(
            &typed,
            source_parameter.type_reference,
            moved_field,
            target_parameter.type_reference,
        )
        .is_none(),
        "a record-level nominal destructor must fence projected cleanup"
    );
}
