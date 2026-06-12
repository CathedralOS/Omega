mod lexicographic;
mod nat;
mod patterns;
mod slice;
mod struct_view;

use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::graph;
use super::order::{AmbiguousDefault, OrderResolution, RankingOrder, decreasing_value_text};

/// The outcome of attempting to prove a terminating machine's `decreases`
/// clause, distinguishing the cases the caller renders as different diagnostics.
pub(super) enum DecreaseOutcome {
    /// The decrease was proven across every cyclic edge.
    Proven,
    /// The order resolved, but the decrease could not be proven.
    Unproven,
    /// A plain `decreases value` clause whose decreasing value has no single
    /// builtin well-founded order; the explicit `-> View` form is required.
    /// Carries the details the diagnostic renders.
    AmbiguousOrder(AmbiguousDefault),
    /// A subtraction-shaped clause written backwards: the declared
    /// `lower - upper` cannot decrease, but the swapped operands prove as the
    /// named bounded distance on every cyclic edge. The diagnostic names the
    /// right shape instead of a bare "cannot prove".
    InvertedDistance(InvertedDistance),
}

/// The operand texts the inverted-distance diagnostic renders: the clause as
/// declared and the corrected `upper - lower` spelling that proves.
pub(super) struct InvertedDistance {
    pub(super) declared: String,
    pub(super) corrected: String,
}

/// Which operand orientation the nat distance prover should read from a
/// subtraction-shaped decreases clause: the declared `left - right`, or the
/// swapped probe used to recognize an inverted bounded distance.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DistanceOrientation {
    Declared,
    Swapped,
}

pub(super) fn machine_decrease_outcome(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> DecreaseOutcome {
    let decreases = program
        .expression_table
        .expression_handles(machine.decreases);
    if decreases.len() != 1 {
        return DecreaseOutcome::Unproven;
    }
    let decreases = decreases[0];

    let states = program.machine_states(machine);
    // The decreases clause is declared on the machine signature, so its names
    // resolve against the machine's root (entry) state.
    let Some(root_state) = states.first() else {
        return DecreaseOutcome::Proven;
    };
    let decrease_order = program.machine_decrease_order(machine.decrease_order);
    let order = match RankingOrder::resolve(program, root_state, decreases, decrease_order) {
        OrderResolution::Resolved(order) => order,
        OrderResolution::AmbiguousDefault(ambiguity) => {
            return DecreaseOutcome::AmbiguousOrder(ambiguity);
        }
        OrderResolution::Unsupported => return DecreaseOutcome::Unproven,
    };

    let adjacency = graph::machine_adjacency(program, machine);
    let components = graph::strongly_connected_components(&adjacency);

    let proven_with = |orientation: DistanceOrientation| {
        components
            .iter()
            .filter(|component| graph::component_is_cyclic(&adjacency, component))
            .all(|component| {
                component_has_proven_decrease(
                    program,
                    machine,
                    &adjacency,
                    component,
                    decreases,
                    &order,
                    orientation,
                )
            })
    };

    if proven_with(DistanceOrientation::Declared) {
        return DecreaseOutcome::Proven;
    }

    // An unproven subtraction-shaped clause is probed with its operands
    // swapped: when the swapped distance proves on every cyclic edge, the
    // clause is the named bounded distance written backwards, and the
    // diagnostic can point at the right shape.
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(decreases)
        && matches!(binary.operator, BinaryOperator::Subtract)
        && matches!(
            order,
            RankingOrder::BoundedDistance | RankingOrder::NatDescending
        )
        && proven_with(DistanceOrientation::Swapped)
    {
        return DecreaseOutcome::InvertedDistance(InvertedDistance {
            declared: decreasing_value_text(program, decreases),
            corrected: format!(
                "{} - {}",
                decreasing_value_text(program, binary.right),
                decreasing_value_text(program, binary.left)
            ),
        });
    }

    DecreaseOutcome::Unproven
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
    orientation: DistanceOrientation,
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
            state_has_proven_supported_self_loop(program, source, decreases, order, orientation)
        } else {
            edge_has_proven_decrease(program, source, target, decreases, order, orientation)
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
    orientation: DistanceOrientation,
) -> bool {
    match order {
        RankingOrder::NatDescending
        | RankingOrder::BoundedDistance
        | RankingOrder::CustomNatDescending => program
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
                    orientation,
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
    orientation: DistanceOrientation,
) -> bool {
    match order {
        RankingOrder::NatDescending
        | RankingOrder::BoundedDistance
        | RankingOrder::CustomNatDescending => {
            nat::state_has_proven_self_loop(program, state, decreases, orientation)
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
