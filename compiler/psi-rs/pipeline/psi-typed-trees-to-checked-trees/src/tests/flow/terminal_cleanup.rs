use super::*;

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn machine_and_entry_state(
    checked: &psi_checked_trees::CheckedTrees,
    machine_name: &str,
) -> (psi_symbols::SymbolHandle, psi_symbols::SymbolHandle) {
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
    let psi_checked_trees::CheckedStructuralUnitControlTerminatorPlan::Jump {
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
    let psi_checked_trees::CheckedStructuralUnitControlTerminatorPlan::Jump {
        scalar_arguments,
        ..
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
        psi_checked_trees::types::PrimitiveType::I32
    );
    assert_eq!(trivial_affine_discard_parameter_positions, &[0]);
    let psi_checked_trees::CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
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
    let psi_checked_trees::CheckedStructuralUnitControlTerminatorPlan::Conditional {
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
    assert_eq!(plan.result_type, psi_typed_trees::types::PrimitiveType::I32);
    assert_eq!(plan.return_statement_ordinal, 0);
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [1, 0]);
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
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [0]);
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
    assert_eq!(
        plan.result_type,
        psi_typed_trees::types::PrimitiveType::Bool
    );
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [0]);
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
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [0]);
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
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [2, 0]);
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
    assert_eq!(
        plan.result_type,
        psi_typed_trees::types::PrimitiveType::Bool
    );
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [0]);
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
            .all(|binding| binding.primitive_type == psi_typed_trees::types::PrimitiveType::Bool)
    );
    assert_eq!(plan.return_statement_ordinal, 7);
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [0]);

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
    assert_eq!(plan.trivial_affine_discard_parameter_positions, [0]);

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
