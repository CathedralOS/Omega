use super::checked_program_result;
use crate::execution::terminal_scalar::finalize_checked_scalar_graph_plans;
use checked_trees::{CheckedScalarStateTerminator, CheckedStructuralControlTransferSourcePlan};

#[test]
fn borrowed_state_parameters_rejoin_each_authored_roster_and_transfer() {
    let checked = checked_program_result(
        r#"
data Context { value: u32; }
machine Context::start(context: &mut Context, other: &mut Context, result: u32) -> u32 {
    transition { _ -> finish(result, other, context) }
    state finish(answer: u32, first: &mut Context, second: &mut Context) { answer }
}
machine invoke(context: &mut Context, other: &mut Context) {
    let answer: u32 = Context::start(context, other, 42);
    context.value = answer;
}
"#,
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Context::start")
        .unwrap()
        .symbol;
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let graph = plans
        .for_machine(machine)
        .unwrap_or_else(|| panic!("borrowed multi-state graph: {:?}", plans.declined));
    assert_eq!(graph.states.len(), 2);
    assert_eq!(graph.states[0].structural_parameters[0].position, 0);
    assert_eq!(graph.states[1].structural_parameters[0].position, 1);
    let CheckedScalarStateTerminator::Jump(successor) = &graph.states[0].terminator else {
        panic!("authored successor");
    };
    let transfers = successor.structural_transfers;
    let retained = plans.structural_transfers.span(transfers).unwrap();
    assert_eq!(retained.len(), 2);
    assert_eq!(
        retained[0].source,
        CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 }
    );
    assert_eq!(
        retained[1].source,
        CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
    );
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke")
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(caller.symbol)
            .is_some(),
        "ordinary callers retain the multi-state scalar target"
    );
    for mutation in 0..3 {
        let mut plans = plans.clone();
        match mutation {
            0 => {
                plans.structural_transfers.span_mut(transfers).unwrap()[0].source =
                    CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
            }
            1 => {
                plans.structural_transfers.span_mut(transfers).unwrap()[0].target_parameter_index =
                    1
            }
            _ => {
                plans
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine)
                    .unwrap()
                    .states[1]
                    .structural_parameters[0]
                    .position = 0
            }
        }
        finalize_checked_scalar_graph_plans(
            &checked,
            &checked.facts.values.scalar_expressions,
            &checked.facts.flow.ownership,
            &checked.facts.values.scalar_computations,
            &mut plans,
            &checked.facts.values.proof_terms,
            &checked.facts.operators,
        );
        assert!(plans.for_machine(machine).is_none(), "mutation {mutation}");
    }
}
