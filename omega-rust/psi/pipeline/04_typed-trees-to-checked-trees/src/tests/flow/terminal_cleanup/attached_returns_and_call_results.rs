use super::{machine_and_entry_state, scalar_discard_positions};
use crate::tests::front_end::checked_program;

#[test]
fn attached_scalar_literal_return_retains_exact_structural_cleanup() {
    let checked = checked_program(
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
    let checked = checked_program(
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
    let checked = checked_program(
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
    let checked = checked_program(
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
    let checked = checked_program(
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
    let checked = checked_program(
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
    let supported = checked_program(
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

    let composed = checked_program(
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

    let rejected = checked_program(
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

#[test]
fn owned_call_result_cleanup_requires_exact_transfer_on_every_edge() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let checked = checked_program(
        r#"
        data Kind { case Missing; case Other; }
        machine make() -> Kind { Kind::Missing }
        machine route(choose: bool) {
            let kind: Kind = make();
            transition choose {
                true -> first(kind)
                false -> second(kind)
            }
            state first(kind: Kind) {}
            state second(kind: Kind) {}
        }
        "#,
    );
    let (machine, state) = machine_and_entry_state(&checked, "route");
    let rebuild = |facts: &checked_trees::CheckFacts| {
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            facts,
        )
    };
    let plans = rebuild(&checked.facts);
    let plan = plans
        .for_state(machine, state)
        .expect("both edges transfer the owned result");
    assert_eq!(plan.edges.len(), 2);
    assert!(
        plan.edges
            .iter()
            .all(|edge| edge.trivial_affine_discard_parameter_positions.is_empty())
    );
    let transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Transfer
        })
        .map(|(handle, event)| (handle, event.clone()))
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 2);
    for (handle, original) in transfers {
        for corruption in [
            "missing edge",
            "duplicate move",
            "wrong origin",
            "wrong target",
            "live obligation",
            "stale path",
            "extra transfer",
        ] {
            let mut changed = checked.facts.clone();
            let permissions = &mut changed.flow.ownership.permissions;
            match corruption {
                "missing edge" => permissions.get_mut(handle).machine_symbol = Default::default(),
                "duplicate move" => {
                    permissions.insert(original.clone());
                }
                "wrong origin" => {
                    permissions.get_mut(handle).provenance = PermissionProvenance::Unknown
                }
                "wrong target" => {
                    permissions.get_mut(handle).source = PermissionEventSource::Call {
                        statement_index: match original.source {
                            PermissionEventSource::Call {
                                statement_index, ..
                            } => statement_index,
                            _ => panic!("transfer call"),
                        },
                        call_ordinal: 0,
                        target_symbol: machine,
                    }
                }
                "live obligation" => permissions.get_mut(handle).obligation_live = true,
                "stale path" => {
                    permissions.get_mut(handle).segments =
                        arena::HandleSpan::from_parts(arena::Handle::invalid(), 1)
                }
                "extra transfer" => {
                    let mut extra = original.clone();
                    extra.source = PermissionEventSource::Statement { statement_index: 0 };
                    permissions.insert(extra);
                }
                _ => unreachable!(),
            }
            assert!(
                rebuild(&changed).for_state(machine, state).is_none(),
                "{corruption} must not publish a partial cleanup plan"
            );
        }
    }
}
