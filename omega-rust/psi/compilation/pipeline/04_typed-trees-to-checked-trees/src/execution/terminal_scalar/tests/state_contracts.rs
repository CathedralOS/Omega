use super::checked_program_result;
use crate::execution::terminal_scalar::finalize_checked_scalar_graph_plans;
use checked_trees::{CheckedBooleanExpression, ClosedScalarContractValue};

#[test]
fn scalar_state_requires_retains_and_replays_authored_predicates() {
    let checked = checked_program_result(
        r#"
        machine start(value: u32) -> u32 {
            transition { _ -> finish(value % 10) }
            state finish(pending: u32) requires pending <= 9 { pending }
        }
    "#,
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "start")
        .unwrap()
        .symbol;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "{:?}",
                checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .declined_reason(machine)
            )
        });
    assert!(graph.states[0].requires.is_empty());
    assert!(matches!(
        graph.states[1].requires.as_slice(),
        [Some(ClosedScalarContractValue::Predicate(
            CheckedBooleanExpression::IntegerComparison { .. }
        ))]
    ));
    for mutation in 0..5 {
        let mut plans = checked.facts.flow.terminal_scalar_graphs.clone();
        let graph = plans
            .machines
            .iter_mut()
            .find(|graph| graph.machine == machine)
            .unwrap();
        match mutation {
            0 => graph.states[1].requires.clear(),
            1 => graph.states[1].requires[0] = None,
            2 => {
                graph.states[1].requires[0] = Some(ClosedScalarContractValue::Predicate(
                    CheckedBooleanExpression::Constant(true),
                ))
            }
            3 => {
                let duplicate = graph.states[1].requires[0].clone();
                graph.states[1].requires.push(duplicate);
            }
            _ => {
                let Some(ClosedScalarContractValue::Predicate(
                    CheckedBooleanExpression::IntegerComparison { right, .. },
                )) = &mut graph.states[1].requires[0]
                else {
                    panic!("comparison predicate");
                };
                let checked_trees::CheckedScalarExpression::IntegerLiteral { literal } =
                    right.as_mut()
                else {
                    panic!("authored upper bound");
                };
                *literal = validation::land_integer_value(
                    &numerics::bignum::BigInt::from_u64(9),
                    typed_trees::types::PrimitiveType::U64,
                )
                .expect("same text, different carrier");
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
