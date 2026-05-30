mod lexicographic;
mod nat;
mod patterns;
mod slice;
mod struct_view;

use omega_typed_trees::expression::ExpressionHandle;

use super::graph;
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
    let decreases = decreases[0];

    let states = program.machine_states(machine);
    // The decreases clause is declared on the machine signature, so its names
    // resolve against the machine's root (entry) state.
    let Some(root_state) = states.first() else {
        return true;
    };
    let decrease_order = program.machine_decrease_order(machine.decrease_order);
    let Some(order) = RankingOrder::from_path(program, root_state, decreases, decrease_order)
    else {
        return false;
    };

    let adjacency = graph::machine_adjacency(program, machine);
    let components = graph::strongly_connected_components(&adjacency);

    components
        .iter()
        .filter(|component| graph::component_is_cyclic(&adjacency, component))
        .all(|component| {
            component_has_proven_decrease(
                program, machine, &adjacency, component, decreases, &order,
            )
        })
}

/// A cyclic component terminates when the measure strictly decreases across
/// every transition edge that stays inside the cycle. Requiring a strict
/// decrease on each in-cycle edge is sufficient (if stronger than necessary)
/// for well-foundedness around the cycle, and it composes the existing
/// single-edge ranking proofs.
fn component_has_proven_decrease(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    adjacency: &[Vec<usize>],
    component: &[usize],
    decreases: ExpressionHandle,
    order: &RankingOrder,
) -> bool {
    let states = program.machine_states(machine);
    let edges = graph::cyclic_edges(adjacency, component);
    if edges.is_empty() {
        return false;
    }

    // A direct self-loop on a single-state component can use the full set of
    // supported orders. Genuine multi-state cycles are proven edge-by-edge with
    // the Nat-descending prover, which is the order whose decrease is defined
    // pointwise across differing source/target states.
    let single_state_self_loop = component.len() == 1 && edges.len() == 1;

    edges.iter().all(|edge| {
        let Some(source) = states.get(edge.from) else {
            return false;
        };
        let Some(target) = states.get(edge.to) else {
            return false;
        };

        if single_state_self_loop {
            state_has_proven_supported_self_loop(program, source, decreases, order)
        } else {
            edge_has_proven_decrease(program, source, target, decreases, order)
        }
    })
}

/// Prove a strict decrease across one cyclic edge between (possibly distinct)
/// states. Only orders with a pointwise cross-state meaning are accepted here.
fn edge_has_proven_decrease(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
    order: &RankingOrder,
) -> bool {
    match order {
        RankingOrder::NatDescending | RankingOrder::CustomNatDescending => program
            .statement_table
            .statements(source.statement_nodes)
            .iter()
            .filter_map(|statement| patterns::guarded_edge_to(program, statement, target.symbol))
            .any(|edge| {
                nat::edge_decrease_proven(
                    program,
                    source,
                    target,
                    edge.guard,
                    edge.arguments,
                    decreases,
                )
            }),
        // Slice-length, struct-view and lexicographic decreases are only
        // proven across direct self-loops; a multi-state cycle using one of
        // these orders is conservatively rejected.
        RankingOrder::SliceLength
        | RankingOrder::CustomStructView(_)
        | RankingOrder::Lexicographic(_) => false,
    }
}

fn state_has_proven_supported_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
    order: &RankingOrder,
) -> bool {
    match order {
        RankingOrder::NatDescending | RankingOrder::CustomNatDescending => {
            nat::state_has_proven_self_loop(program, state, decreases)
        }
        RankingOrder::SliceLength => slice::state_has_proven_self_loop(program, state, decreases),
        RankingOrder::CustomStructView(field) => {
            struct_view::state_has_proven_self_loop(program, state, decreases, field)
        }
        RankingOrder::Lexicographic(fields) => {
            lexicographic::state_has_proven_self_loop(program, state, decreases, fields)
        }
    }
}
