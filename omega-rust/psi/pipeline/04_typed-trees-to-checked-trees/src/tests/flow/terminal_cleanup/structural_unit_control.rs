use crate::tests::front_end::checked_program;

#[test]
fn structural_unit_jump_composes_signatures_transfers_and_cleanup() {
    let supported = checked_program(
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
    assert_eq!(
        transfers[0].source,
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 }
    );
    assert_eq!(transfers[0].target_parameter_index, 0);
    let checked_trees::CheckedStructuralUnitControlTerminatorPlan::Jump {
        scalar_arguments, ..
    } = &plan.states[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(scalar_arguments.len(), 1);
    assert_eq!(scalar_arguments[0].argument_ordinal, 1);
    assert_eq!(
        scalar_arguments[0].source,
        checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 0 }
    );
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

    let rejected = checked_program(
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
    let admitted = checked_program(source);
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
        when_true.scalar_arguments[0].source,
        checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 0 }
    );
    assert_eq!(
        when_true.scalar_arguments[0].target_scalar_parameter_index,
        0
    );

    let without_witness =
        checked_program(&source.replace("terminates by remaining -> Nat::Descending;", ""));
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
    let admitted = checked_program(
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
                source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                    index: 0
                },
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
    let supported = checked_program(
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
    assert_eq!(
        when_true.transfers[0].source,
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
    );
    assert_eq!(when_true.scalar_arguments.len(), 1);
    assert_eq!(when_true.scalar_arguments[0].argument_ordinal, 1);
    assert_eq!(
        when_true.scalar_arguments[0].source,
        checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 1 }
    );
    assert_eq!(
        when_true.scalar_arguments[0].target_scalar_parameter_index,
        0
    );
    assert_eq!(when_true.trivial_affine_discard_parameter_positions, [1]);
    assert_eq!(when_false.statement_ordinal, 1);
    assert_eq!(
        when_false.transfers[0].source,
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 }
    );
    assert_eq!(when_false.scalar_arguments, when_true.scalar_arguments);
    assert_eq!(when_false.trivial_affine_discard_parameter_positions, [0]);

    let rejected = checked_program(
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
    let supported = checked_program(
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

    let nested = checked_program(
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

    let rejected = checked_program(
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
    let checked = checked_program(
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
