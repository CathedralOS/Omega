use super::super::{
    CheckedScalarBranchDestination, CheckedScalarMachineGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor,
};
use crate::execution::terminal_scalar::finalize_checked_scalar_graph_plans;
use crate::tests::front_end::checked_program_result;
use checked_trees::CheckedStructuralControlTransferSourcePlan;
use typed_trees::types::PrimitiveType;

const SOURCE: &str = r#"
machine reset(value: &mut u64) -> u64 { value = 0; 0 }
data Limits { limit: u64; divisor: u64 [3..=5]; }
machine walk(remaining: u64 [0..=5], limits: Limits, marker: u64)
terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);
-> u64 {
    let mut scratch: u64 = 0;
    transition remaining > 0 {
        true -> walk(remaining - 1, limits, reset(&mut scratch))
        false -> remaining
    }
}
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program_result(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

fn machine(checked: &checked_trees::CheckedTrees) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap()
        .symbol
}

fn successor(graph: &CheckedScalarMachineGraph) -> &CheckedScalarSuccessor {
    let CheckedScalarStateTerminator::Conditional {
        when_true: CheckedScalarBranchDestination::Jump(successor),
        when_false: CheckedScalarBranchDestination::Return { .. },
        ..
    } = &graph.states[0].terminator
    else {
        panic!("guarded self-loop and return")
    };
    successor
}

#[test]
fn cyclic_successor_rejects_substituted_argument_rows_and_rank_coordinates() {
    let checked = checked(SOURCE);
    let machine = machine(&checked);
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let original = successor(plans.for_machine(machine).unwrap());
    for mutation in 0..8 {
        let mut plans = plans.clone();
        match mutation {
            0 => {
                plans
                    .structural_transfers
                    .span_mut(original.structural_transfers)
                    .unwrap()[0]
                    .source = CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 }
            }
            1 => {
                plans
                    .structural_transfers
                    .span_mut(original.structural_transfers)
                    .unwrap()[0]
                    .target_parameter_index = 1
            }
            2 => {
                plans
                    .scalar_arguments
                    .span_mut(original.scalar_arguments)
                    .unwrap()[1]
                    .argument_ordinal = 1
            }
            3 => {
                plans
                    .scalar_arguments
                    .span_mut(original.scalar_arguments)
                    .unwrap()[1]
                    .target_scalar_parameter_index = 0
            }
            4 => {
                plans
                    .scalar_arguments
                    .span_mut(original.scalar_arguments)
                    .unwrap()[0]
                    .primitive_type = PrimitiveType::Bool
            }
            5 => {
                plans
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine)
                    .unwrap()
                    .ranked_scc = None
            }
            6 => {
                plans
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine)
                    .unwrap()
                    .ranked_scc
                    .as_mut()
                    .unwrap()
                    .rank_scalar_parameter_index = 1
            }
            _ => {
                plans
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine)
                    .unwrap()
                    .ranked_scc
                    .as_mut()
                    .unwrap()
                    .covered_cyclic_edges[0]
                    .statement_ordinal = 2
            }
        }
        finalize_checked_scalar_graph_plans(
            &checked,
            &checked.facts.values.scalar_expressions,
            &checked.facts.flow.ownership,
            &checked.facts.values.scalar_computations,
            &mut plans,
            &checked.facts.values.proof_terms,
        );
        assert!(plans.for_machine(machine).is_none(), "mutation {mutation}");
    }
}
