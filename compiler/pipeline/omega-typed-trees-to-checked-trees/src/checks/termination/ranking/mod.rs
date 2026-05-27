mod nat;
mod patterns;
mod slice;

use omega_typed_trees::expression::ExpressionHandle;

use super::order::RankingOrder;

pub(super) fn machine_has_proven_supported_decrease(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> bool {
    let decreases = program
        .expression_table
        .expression_handles(machine.decreases);
    if decreases.len() != 1 {
        return false;
    }
    program
        .machine_states(machine)
        .iter()
        .filter(|state| patterns::state_has_direct_self_loop(program, state))
        .all(|state| {
            let decrease_order = program.machine_decrease_order(machine.decrease_order);
            let Some(order) = RankingOrder::from_path(program, state, decreases[0], decrease_order)
            else {
                return false;
            };

            state_has_proven_supported_self_loop(program, state, decreases[0], order)
        })
}

fn state_has_proven_supported_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
    order: RankingOrder,
) -> bool {
    match order {
        RankingOrder::NatDescending | RankingOrder::CustomNatDescending => {
            nat::state_has_proven_self_loop(program, state, decreases)
        }
        RankingOrder::SliceLength => slice::state_has_proven_self_loop(program, state, decreases),
    }
}
