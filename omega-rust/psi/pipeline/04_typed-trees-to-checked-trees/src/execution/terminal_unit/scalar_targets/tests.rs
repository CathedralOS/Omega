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
            5 => graph.states[0].result_type = PrimitiveType::Bool,
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
