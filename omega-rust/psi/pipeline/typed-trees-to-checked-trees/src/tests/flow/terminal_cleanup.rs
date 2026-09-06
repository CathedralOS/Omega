use super::*;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn typed_program(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn scalar_discard_positions(
    plan: &checked_trees::CheckedStructuralScalarReturnMachinePlan,
) -> Vec<u32> {
    plan.cleanup_actions
        .iter()
        .filter_map(|action| match action {
            checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(position) => {
                Some(*position)
            }
            checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_) => None,
        })
        .collect()
}

fn machine_and_entry_state(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
) -> (symbols::SymbolHandle, symbols::SymbolHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(machine_name))
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    let state = checked
        .machine_states(machine)
        .first()
        .unwrap_or_else(|| panic!("machine `{machine_name}` has no entry state"));
    (machine.symbol, state.symbol)
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
        crate::flow::build_checked_structural_control_cleanup_plans(&checked.typed, &checked.facts);
    assert_eq!(
        rebuilt, checked.facts.flow.terminal_structural_control_cleanups,
        "the checked edge plan is reconstructed from typed ownership evidence"
    );
    let mut missing_evidence = checked.facts.clone();
    missing_evidence.flow.ownership.permissions = Default::default();
    assert!(
        crate::flow::build_checked_structural_control_cleanup_plans(
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
    let checked = checked(
        r#"
        data Token { value: i32; }

        machine route(input: Token) -> i32
        {
            let local: Token = Token { value: 1 };
            transition { _ -> next(input) }
            state next(input: Token) -> i32 { 0 }
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
        "a state needing local cleanup must not publish a partial parameter-only plan"
    );
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
        assert_eq!(sibling.source_parameter_index, 0);
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

        let rebuilt = crate::flow::build_checked_structural_control_cleanup_plans(
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
            crate::flow::build_checked_structural_control_cleanup_plans(
                &checked.typed,
                &checked.facts,
            ),
            "transfer type and residual-path drift must not match reconstruction"
        );

        let mut missing_exit = checked.facts.clone();
        missing_exit.flow.ownership.permissions = Default::default();
        assert!(
            crate::flow::build_checked_structural_control_cleanup_plans(
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
        crate::flow::exact_two_field_record_projection_for_test(
            &typed,
            source_parameter.type_reference,
            moved_field,
            target_parameter.type_reference,
        )
        .is_none(),
        "a record-level nominal destructor must fence projected cleanup"
    );
}

#[test]
fn structural_unit_jump_composes_signatures_transfers_and_cleanup() {
    let supported = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(first: Token, second: Token, value: i32)
        {
            transition { _ -> next(second, value) }
            state next(second: Token, value: i32) {}
        }
        "#,
    );
    let machine = supported
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    let plan = supported
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(machine)
        .expect("the exact structural Unit graph should compose");
    assert_eq!(plan.states.len(), 2);
    assert_eq!(plan.states[0].scalar_parameters.len(), 1);
    assert_eq!(plan.states[1].scalar_parameters.len(), 1);
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::Jump {
        transfers,
        trivial_affine_discard_parameter_positions,
        ..
    } = &plan.states[0].terminator
    else {
        panic!("entry state should jump")
    };
    assert_eq!(transfers.len(), 1);
    assert_eq!(transfers[0].source_parameter_index, 1);
    assert_eq!(transfers[0].target_parameter_index, 0);
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::Jump {
        scalar_arguments, ..
    } = &plan.states[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(scalar_arguments.len(), 1);
    assert_eq!(scalar_arguments[0].argument_ordinal, 1);
    assert_eq!(scalar_arguments[0].source_scalar_parameter_index, 0);
    assert_eq!(scalar_arguments[0].target_scalar_parameter_index, 0);
    assert_eq!(
        scalar_arguments[0].primitive_type,
        checked_trees::types::PrimitiveType::I32
    );
    assert_eq!(trivial_affine_discard_parameter_positions, &[0]);
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
        trivial_affine_discard_parameter_positions,
    } = &plan.states[1].terminator
    else {
        panic!("leaf state should return Unit")
    };
    assert_eq!(trivial_affine_discard_parameter_positions, &[0]);

    let rejected = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(first: Token, second: Token, value: i32)
        {
            transition { _ -> next(second, value == 1) }
            state next(second: Token, matches: bool) {}
        }
        "#,
    );
    let machine = rejected
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    assert!(
        rejected
            .facts
            .flow
            .terminal_structural_unit_controls
            .for_machine(machine)
            .is_none(),
        "computed scalar jump arguments remain outside the direct-input slice"
    );
}

#[test]
fn structural_unit_countdown_retains_exact_ranked_scc_plan() {
    let source = r#"
        data Token { value: i32; }
        data Root {}

        machine Root::countdown(token: Token, remaining: u32)
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> countdown(token, remaining - 1)
                _ -> done(token)
            }
            state done(token: Token) {}
        }
        "#;
    let admitted = checked(source);
    let machine = admitted
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("countdown"))
        .expect("countdown machine")
        .symbol;
    let plan = admitted
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(machine)
        .expect("the ranked structural Unit countdown should compose");
    let ranked = plan.ranked_scc.as_ref().expect("retained ranked SCC");
    assert_eq!(ranked.header_state, plan.states[0].state);
    assert_eq!(ranked.rank_scalar_parameter_index, 0);
    assert_eq!(
        ranked.rank_primitive_type,
        typed_trees::types::PrimitiveType::U32
    );
    assert_eq!(ranked.rank_lower_bound, 0);
    assert_eq!(ranked.rank_upper_bound, u128::from(u32::MAX));
    let [edge] = ranked.covered_cyclic_edges.as_slice() else {
        panic!("one ranked backedge")
    };
    assert_eq!(edge.source_state, ranked.header_state);
    assert_eq!(edge.target_state, ranked.header_state);
    assert_eq!(edge.statement_ordinal, 0);
    assert_eq!(
        edge.guard,
        checked_trees::CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
            scalar_parameter_index: 0,
            primitive_type: typed_trees::types::PrimitiveType::U32,
        }
    );
    assert_eq!(
        edge.successor_argument,
        checked_trees::CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
            argument_ordinal: 1,
            source_scalar_parameter_index: 0,
            target_scalar_parameter_index: 0,
            primitive_type: typed_trees::types::PrimitiveType::U32,
        }
    );
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::Conditional {
        guard_scalar_parameter_index,
        when_true,
        ..
    } = &plan.states[0].terminator
    else {
        panic!("countdown header should retain its conditional")
    };
    assert_eq!(*guard_scalar_parameter_index, 0);
    assert_eq!(when_true.target_state, ranked.header_state);
    assert_eq!(
        when_true.scalar_arguments[0].source_scalar_parameter_index,
        0
    );
    assert_eq!(
        when_true.scalar_arguments[0].target_scalar_parameter_index,
        0
    );

    let without_witness =
        checked(&source.replace("terminates by remaining -> Nat::Descending;", ""));
    let machine = without_witness
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("countdown"))
        .expect("unranked countdown machine")
        .symbol;
    assert!(
        without_witness
            .facts
            .flow
            .terminal_structural_unit_controls
            .for_machine(machine)
            .is_none(),
        "a cyclic structural plan without checker-owned rank evidence must be omitted"
    );
}

#[test]
fn structural_unit_countdown_retains_implicit_mutable_receiver_custody() {
    let admitted = checked(
        r#"
        data Root { value: i32; }

        machine Root::countdown(&mut self, remaining: u32)
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> countdown(remaining - 1)
                _ -> done()
            }
            state done(&mut self) {}
        }
        "#,
    );
    let machine = admitted
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("countdown"))
        .expect("countdown machine")
        .symbol;
    let plan = admitted
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(machine)
        .expect("the ranked mutable-receiver countdown should compose");
    let [header, done] = plan.states.as_slice() else {
        panic!("header and exit state")
    };
    let [header_receiver] = header.structural_parameters.as_slice() else {
        panic!("one header receiver")
    };
    let [done_receiver] = done.structural_parameters.as_slice() else {
        panic!("one exit receiver")
    };
    assert!(header_receiver.is_self);
    assert_eq!(header_receiver, done_receiver);
    assert_eq!(
        header_receiver.access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert_eq!(
        header_receiver.multiplicity,
        language_semantics::Multiplicity::Unrestricted
    );
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &header.terminator
    else {
        panic!("countdown header is conditional")
    };
    for successor in [when_true, when_false] {
        assert_eq!(
            successor.transfers,
            [checked_trees::CheckedStructuralControlTransferPlan {
                source_parameter_index: 0,
                target_parameter_index: 0,
            }]
        );
        assert!(
            successor
                .trivial_affine_discard_parameter_positions
                .is_empty()
        );
    }
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
        trivial_affine_discard_parameter_positions,
    } = &done.terminator
    else {
        panic!("countdown exit returns Unit")
    };
    assert!(trivial_affine_discard_parameter_positions.is_empty());
}

#[test]
fn structural_unit_conditional_composes_independent_transfer_cleanup_frontiers() {
    let supported = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(first: Token, second: Token, choose_first: bool, value: i32)
        {
            transition choose_first {
                true -> keep_first(first, value)
                _ -> keep_second(second, value)
            }
            state keep_first(first: Token, value: i32) {}
            state keep_second(second: Token, value: i32) {}
        }
        "#,
    );
    let machine = supported
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    let plan = supported
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(machine)
        .expect("the exact structural Unit conditional should compose");
    assert_eq!(plan.states.len(), 3);
    assert_eq!(plan.states[0].scalar_parameters.len(), 2);
    assert_eq!(plan.states[1].scalar_parameters.len(), 1);
    assert_eq!(plan.states[2].scalar_parameters.len(), 1);
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::Conditional {
        guard_scalar_parameter_index,
        when_true,
        when_false,
    } = &plan.states[0].terminator
    else {
        panic!("entry state should select two structural successors")
    };
    assert_eq!(*guard_scalar_parameter_index, 0);
    assert_eq!(when_true.statement_ordinal, 0);
    assert_eq!(when_true.transfers[0].source_parameter_index, 0);
    assert_eq!(when_true.scalar_arguments.len(), 1);
    assert_eq!(when_true.scalar_arguments[0].argument_ordinal, 1);
    assert_eq!(
        when_true.scalar_arguments[0].source_scalar_parameter_index,
        1
    );
    assert_eq!(
        when_true.scalar_arguments[0].target_scalar_parameter_index,
        0
    );
    assert_eq!(when_true.trivial_affine_discard_parameter_positions, [1]);
    assert_eq!(when_false.statement_ordinal, 1);
    assert_eq!(when_false.transfers[0].source_parameter_index, 1);
    assert_eq!(when_false.scalar_arguments, when_true.scalar_arguments);
    assert_eq!(when_false.trivial_affine_discard_parameter_positions, [0]);

    let rejected = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(first: Token, second: Token, choose_first: bool)
        {
            transition choose_first == true {
                true -> keep_first(first)
                _ -> keep_second(second)
            }
            state keep_first(first: Token) {}
            state keep_second(second: Token) {}
        }
        "#,
    );
    let machine = rejected
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    assert!(
        rejected
            .facts
            .flow
            .terminal_structural_unit_controls
            .for_machine(machine)
            .is_none(),
        "computed conditional guards remain fail-closed"
    );
}

#[test]
fn structural_unit_bounded_conditional_topology_composes_exact_frontiers() {
    let supported = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(first: Token, second: Token, choose_first: bool, value: i32)
        {
            transition { _ -> decide(first, second, choose_first, value) }
            state decide(first: Token, second: Token, choose_first: bool, value: i32) {
                transition choose_first {
                    true -> keep_first(first, value)
                    _ -> keep_second(second, value)
                }
            }
            state keep_first(first: Token, value: i32) {}
            state keep_second(second: Token, value: i32) {}
        }
        "#,
    );
    let machine = supported
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    let plan = supported
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(machine)
        .expect("one nonentry structural Unit conditional should compose");
    assert_eq!(plan.states.len(), 4);
    assert!(matches!(
        &plan.states[0].terminator,
        checked_trees::CheckedStructuralUnitControlTerminatorPlan::Jump {
            transfers,
            scalar_arguments,
            trivial_affine_discard_parameter_positions,
            ..
        } if transfers.len() == 2
            && scalar_arguments.len() == 2
            && trivial_affine_discard_parameter_positions.is_empty()
    ));
    assert!(matches!(
        &plan.states[1].terminator,
        checked_trees::CheckedStructuralUnitControlTerminatorPlan::Conditional {
            guard_scalar_parameter_index: 0,
            ..
        }
    ));

    let nested = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(
            first: Token,
            second: Token,
            third: Token,
            choose_first: bool,
            choose_second: bool,
            value: i32
        ) {
            transition choose_first {
                true -> keep_first(first, value)
                _ -> decide_second(second, third, choose_second, value)
            }
            state decide_second(
                second: Token,
                third: Token,
                choose_second: bool,
                value: i32
            ) {
                transition choose_second {
                    true -> keep_second(second, value)
                    _ -> keep_third(third, value)
                }
            }
            state keep_first(first: Token, value: i32) {}
            state keep_second(second: Token, value: i32) {}
            state keep_third(third: Token, value: i32) {}
        }
        "#,
    );
    let machine = nested
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    let plan = nested
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(machine)
        .expect("one nested conditional should compose");
    assert_eq!(
        plan.states
            .iter()
            .filter(|state| matches!(
                state.terminator,
                checked_trees::CheckedStructuralUnitControlTerminatorPlan::Conditional { .. }
            ))
            .count(),
        2
    );

    let rejected = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(
            first: Token,
            second: Token,
            third: Token,
            fourth: Token,
            choose_first: bool,
            choose_second: bool,
            choose_third: bool,
            value: i32
        ) {
            transition choose_first {
                true -> keep_first(first, value)
                _ -> decide_second(second, third, fourth, choose_second, choose_third, value)
            }
            state decide_second(
                second: Token,
                third: Token,
                fourth: Token,
                choose_second: bool,
                choose_third: bool,
                value: i32
            ) {
                transition choose_second {
                    true -> keep_second(second, value)
                    _ -> decide_third(third, fourth, choose_third, value)
                }
            }
            state decide_third(
                third: Token,
                fourth: Token,
                choose_third: bool,
                value: i32
            ) {
                transition choose_third {
                    true -> keep_third(third, value)
                    _ -> keep_fourth(fourth, value)
                }
            }
            state keep_first(first: Token, value: i32) {}
            state keep_second(second: Token, value: i32) {}
            state keep_third(third: Token, value: i32) {}
            state keep_fourth(fourth: Token, value: i32) {}
        }
        "#,
    );
    let machine = rejected
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    assert!(
        rejected
            .facts
            .flow
            .terminal_structural_unit_controls
            .for_machine(machine)
            .is_none(),
        "a third conditional state remains outside the bounded topology slice"
    );
}

#[test]
fn structural_unit_diamond_retains_one_join_and_exact_scalar_edges() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::route(
            token: Token,
            choose_left: bool,
            left_value: i32,
            right_value: i32
        ) {
            transition choose_left {
                true -> left(token, left_value)
                _ -> right(token, right_value)
            }
            state left(token: Token, value: i32) {
                transition { _ -> join(token, value) }
            }
            state right(token: Token, value: i32) {
                transition { _ -> join(token, value) }
            }
            state join(token: Token, value: i32) {}
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("route"))
        .expect("route machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(machine)
        .expect("one structural Unit diamond should retain exact edge maps");
    assert_eq!(plan.states.len(), 4);
    let join = plan.states[3].state;
    for state in &plan.states[1..3] {
        assert!(matches!(
            &state.terminator,
            checked_trees::CheckedStructuralUnitControlTerminatorPlan::Jump {
                target_state,
                transfers,
                scalar_arguments,
                trivial_affine_discard_parameter_positions,
                ..
            } if *target_state == join
                && transfers.len() == 1
                && scalar_arguments.len() == 1
                && trivial_affine_discard_parameter_positions.is_empty()
        ));
    }
}

#[test]
fn attached_scalar_literal_return_retains_exact_structural_cleanup() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Root {}

        machine Root::measure(first: Token, second: Token) -> i32
        {
            7i32
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("closed scalar return should compose with structural cleanup");
    assert_eq!(plan.structural_parameters.len(), 2);
    assert_eq!(plan.result_type, typed_trees::types::PrimitiveType::I32);
    assert_eq!(plan.return_statement_ordinal, 0);
    assert_eq!(scalar_discard_positions(plan), [1, 0]);
}

#[test]
fn attached_closed_integer_expression_retains_exact_structural_cleanup() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(token: Token) -> i32 { 3i32 + 4i32 }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("closed integer expression should compose with structural cleanup");
    assert_eq!(scalar_discard_positions(plan), [0]);
}

#[test]
fn attached_closed_branch_free_boolean_retains_exact_structural_cleanup() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(token: Token) -> bool { !(3i32 < 4i32 == false) }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("closed branch-free Boolean should compose with structural cleanup");
    assert_eq!(plan.result_type, typed_trees::types::PrimitiveType::Bool);
    assert_eq!(scalar_discard_positions(plan), [0]);
}

#[test]
fn attached_branch_free_scalar_locals_retain_exact_structural_cleanup() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(token: Token) -> bool
        {
            let base: i32 = 3i32 + 4i32;
            let small: bool = base < 8i32;
            small == true
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("branch-free scalar local prefix should compose with structural cleanup");
    assert_eq!(plan.bindings.len(), 2);
    assert_eq!(plan.bindings[0].statement_ordinal, 0);
    assert_eq!(plan.bindings[1].statement_ordinal, 1);
    assert_eq!(plan.return_statement_ordinal, 2);
    assert_eq!(scalar_discard_positions(plan), [0]);
}

#[test]
fn structural_scalar_return_retains_interleaved_scalar_parameter_map() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(
            first: Token,
            offset: i32,
            second: Token,
            choose: bool
        ) -> bool
        {
            !choose
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("mixed state parameters should publish an exact source-position partition");
    assert_eq!(plan.structural_parameters.len(), 2);
    assert_eq!(plan.structural_parameters[0].position, 0);
    assert_eq!(plan.structural_parameters[1].position, 2);
    assert_eq!(plan.scalar_parameters.len(), 2);
    assert_eq!(plan.scalar_parameters[0].source_position, 1);
    assert_eq!(plan.scalar_parameters[1].source_position, 3);
    assert_eq!(scalar_discard_positions(plan), [2, 0]);
}

#[test]
fn structural_scalar_return_retains_short_circuit_return_cleanup() {
    let checked = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(token: Token) -> bool { true && false }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("short-circuit return should retain cleanup for every terminal leaf");
    assert_eq!(plan.result_type, typed_trees::types::PrimitiveType::Bool);
    assert_eq!(scalar_discard_positions(plan), [0]);
}

#[test]
fn structural_scalar_return_supports_repeated_carried_short_circuit_local_continuations() {
    let supported = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(token: Token) -> bool
        {
            let seed: bool = true;
            let first: bool = seed && false;
            let first_middle: bool = !first;
            let second: bool = first_middle || false;
            let second_middle: bool = !second;
            let third: bool = second_middle && true;
            let inverted: bool = !third;
            inverted
        }
        "#,
    );
    let machine = supported
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = supported
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("branch-free scalar work may surround repeated short-circuit continuations");
    assert_eq!(plan.bindings.len(), 7);
    assert!(
        plan.bindings
            .iter()
            .all(|binding| binding.primitive_type == typed_trees::types::PrimitiveType::Bool)
    );
    assert_eq!(plan.return_statement_ordinal, 7);
    assert_eq!(scalar_discard_positions(plan), [0]);

    let composed = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(token: Token) -> bool
        {
            let first: bool = true && false;
            let middle: bool = !first;
            let second: bool = middle || false;
            second && true
        }
        "#,
    );
    let machine = composed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    let plan = composed
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine)
        .expect("repeated local decisions may feed one final short-circuit return");
    assert_eq!(plan.bindings.len(), 3);
    assert_eq!(plan.return_statement_ordinal, 3);
    assert_eq!(scalar_discard_positions(plan), [0]);

    let rejected = checked(
        r#"
        data Token { value: i32; }
        data Root {}
        machine Root::measure(token: Token) -> bool
        {
            let mut staged: bool = true && false;
            staged
        }
        "#,
    );
    let machine = rejected
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("measure"))
        .expect("measure machine")
        .symbol;
    assert!(
        rejected
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine)
            .is_none(),
        "mutable short-circuit local stages remain fail-closed"
    );
}
