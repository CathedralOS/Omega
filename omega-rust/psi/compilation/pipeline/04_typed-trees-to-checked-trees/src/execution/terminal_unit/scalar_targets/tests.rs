use super::super::CheckedStructuralAccess;
use super::{PrimitiveType, SymbolHandle};
use crate::execution::terminal_unit::scalar_targets::registered_structural_graph_target;
use crate::tests::front_end::checked_program_result;

const SOURCE: &str = r#"
machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
machine first(left: u64, right: u64) -> u64 { left }
machine enter(before: u64, slot: &mut u64, after: u64) -> u64 {
    let answer: u64 = first(stamp(&mut slot, before), stamp(&mut slot, after));
    answer
}
machine observe(output: &mut u64) {
    let answer: u64 = enter(7, &mut output, 11);
    output = answer;
}
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn machine_symbol(checked: &checked_trees::CheckedTrees, name: &str) -> SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .symbol
}

#[test]
fn mixed_graph_target_rejects_stale_parameter_and_shape_namespaces() {
    let checked = checked(SOURCE);
    let enter = machine_symbol(&checked, "enter");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(enter)
        .unwrap();
    let state = graph.states[0].state;
    assert!(
        registered_structural_graph_target(
            &checked.typed,
            &checked.facts,
            Some(crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &checked.facts.flow.terminal_structural_scalar_returns
            }),
            enter,
            state,
            PrimitiveType::U64
        )
        .is_some()
    );
    for mutation in 0..6 {
        let mut facts = checked.facts.clone();
        let graph = facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == enter)
            .unwrap();
        match mutation {
            0 => graph.states[0].scalar_parameters[1].source_position = 1,
            1 => graph.states[0].structural_parameters[0].position = 2,
            2 => graph.states[0].parameter_types[0] = PrimitiveType::Bool,
            3 => {
                graph.states[0].structural_parameters[0].access =
                    CheckedStructuralAccess::SharedBorrow
            }
            4 => {
                let _ = graph.states[0].scalar_parameters.pop();
            }
            5 => graph.states[0].result_type = Some(PrimitiveType::Bool),
            _ => unreachable!(),
        }
        assert!(
            registered_structural_graph_target(
                &checked.typed,
                &facts,
                Some(crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns
                }),
                enter,
                state,
                PrimitiveType::U64
            )
            .is_none(),
            "mutation {mutation}"
        );
    }
    let mut facts = checked.facts.clone();
    facts.flow.terminal_scalar_graphs.structural_types.clear();
    assert!(
        registered_structural_graph_target(
            &checked.typed,
            &facts,
            Some(crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns
            }),
            enter,
            state,
            PrimitiveType::U64
        )
        .is_none()
    );
    let mut facts = checked.facts.clone();
    facts
        .flow
        .terminal_scalar_graphs
        .machines
        .push(graph.clone());
    assert!(
        registered_structural_graph_target(
            &checked.typed,
            &facts,
            Some(crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns
            }),
            enter,
            state,
            PrimitiveType::U64
        )
        .is_none()
    );
}

#[test]
fn qualified_static_scalar_body_preserves_its_explicit_signature() {
    let checked = checked(
        r#"
        boundary trait Output { machine observe(value: i32) reaches Output; }
        data Score { points: i32 in Wrapping; }
        machine Score::bump(score: &mut Score, by: i32 in Wrapping) {
            score.points = score.points + by;
        }
        machine Score::read(score: &Score) -> i32 {
            transition { _ -> (score.points as i32) }
        }
        machine entry() reaches Output {
            let mut score: Score;
            Score::bump(&mut score, 70);
            let seen: i32 = Score::read(&score);
            Output::observe(seen);
        }
        "#,
    );
    assert_registered_explicit_signature(&checked, 1);
}

#[test]
fn qualified_static_scalar_graph_keeps_explicit_state_arguments() {
    let checked = checked(
        r#"
        boundary trait Output { machine observe(value: i32) reaches Output; }
        data Score { points: i32 in Wrapping; }
        machine Score::read(score: &Score) -> i32 {
            transition { _ -> finish(score) }
            state finish(score: &Score) -> i32 {
                transition { _ -> (score.points as i32) }
            }
        }
        machine entry(score: &Score) reaches Output {
            let seen: i32 = Score::read(score);
            Output::observe(seen);
        }
        "#,
    );
    assert_registered_explicit_signature(&checked, 2);
}

fn assert_registered_explicit_signature(checked: &checked_trees::CheckedTrees, states: usize) {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let caller = plans
        .for_machine(machine_symbol(checked, "entry"))
        .expect("a qualified static scalar graph must not prune its caller");
    let target = machine_symbol(checked, "Score::read");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(target)
        .expect("registered scalar graph");
    assert_eq!(graph.states.len(), states);
    assert!(
        graph.states.iter().all(|state| {
            matches!(state.structural_parameters.as_slice(), [parameter]
            if !parameter.is_self && parameter.position == 0
                && parameter.access == CheckedStructuralAccess::SharedBorrow)
        }),
        "qualification retains an explicit score loan in every state"
    );
    let operation = caller
        .operations
        .iter()
        .find(|operation| {
            operation
                .call()
                .is_some_and(|call| call.target.machine == target)
        })
        .expect("the caller retains its scalar call");
    let available =
        |checked: &checked_trees::CheckedTrees,
         operation: &checked_trees::CheckedUnitEffectOperationPlan| {
            let plans = &checked.facts.flow.terminal_unit_effects;
            super::available_target(
                &checked.typed,
                &checked.facts,
                crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
                },
                &plans.machines,
                &plans.composed_machines,
                &super::ScalarCallSite::of_plan(caller),
                operation,
            )
        };
    assert!(matches!(
        available(checked, operation),
        Some(super::AvailableScalarTarget::Registered)
    ));
    // The selected graph owns signature custody; mutating an unused Unit
    // candidate would no longer test this route. Qualification cannot invent
    // an implicit receiver, and each call still owns its exact contract.
    for mutation in [
        "attachment",
        "access",
        "position",
        "contract",
        "caller access",
    ] {
        let mut changed = checked.clone();
        let mut changed_operation = operation.clone();
        let selected = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == target)
            .expect("selected graph");
        match mutation {
            "attachment" => selected.states[0].structural_parameters[0].is_self = true,
            "access" => {
                selected.states[0].structural_parameters[0].access = CheckedStructuralAccess::Owned
            }
            "position" => selected.states[0].structural_parameters[0].position += 1,
            "contract" => {
                changed_operation
                    .call_mut()
                    .unwrap()
                    .target
                    .contract_report_fingerprint ^= 1
            }
            "caller access" => {
                changed_operation.call_mut().unwrap().structural_arguments[0].access =
                    CheckedStructuralAccess::Owned
            }
            _ => unreachable!(),
        }
        assert!(
            available(&changed, &changed_operation).is_none(),
            "{mutation}"
        );
    }
}
