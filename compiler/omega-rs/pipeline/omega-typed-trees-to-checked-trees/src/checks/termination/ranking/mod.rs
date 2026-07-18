mod lexicographic;
mod nat;
mod patterns;
mod slice;
mod struct_view;

use omega_typed_trees::expression::ExpressionHandle;

use super::graph;
use super::order::{AmbiguousDefault, OrderResolution, RankingOrder, decreasing_value_text};

/// The outcome of attempting to prove a machine's private ranking witness
/// clause, distinguishing the cases the caller renders as different diagnostics.
pub(super) enum DecreaseOutcome {
    /// The decrease was proven across every cyclic edge.
    Proven,
    /// The order resolved, but the decrease could not be proven.
    Unproven,
    /// A plain `terminates by value` witness whose subject has no single
    /// builtin well-founded order; the explicit `-> View` form is required.
    /// Carries the details the diagnostic renders.
    AmbiguousOrder(AmbiguousDefault),
    /// A two-subject bounded distance written backwards: the declared
    /// `(upper, lower)` tuple cannot decrease, but the swapped subjects prove
    /// as the named bounded distance on every cyclic edge. The diagnostic
    /// names the right shape instead of a bare "cannot prove".
    InvertedDistance(InvertedDistance),
    /// The decrease itself proved, but the authored `in ...` interval could
    /// not be shown to contain every rank produced by the selected view.
    UnprovenRange(String),
}

/// The subject texts the inverted-distance diagnostic renders: the clause as
/// declared and the corrected `(lower, upper)` spelling that proves.
pub(super) struct InvertedDistance {
    pub(super) declared: String,
    pub(super) corrected: String,
}

/// Which subject orientation the nat distance prover should read from a
/// two-subject decreases tuple: the declared `(lower, upper)`, or the swapped
/// probe used to recognize an inverted bounded distance.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DistanceOrientation {
    Declared,
    Swapped,
}

/// The ranked subjects a ranking witness threads into the edge provers: a
/// single decreasing value, or the two-subject bounded distance whose subjects
/// bind in order to `Nat::BoundedDistance`'s `(lower, upper)` parameters.
#[derive(Clone, Copy)]
pub(super) enum DecreaseMeasure {
    Single(ExpressionHandle),
    Distance {
        lower: ExpressionHandle,
        upper: ExpressionHandle,
    },
}

pub(super) fn machine_decrease_outcome(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> DecreaseOutcome {
    let subjects = program
        .expression_table
        .expression_handles(machine.ranking_witness.subjects);
    let authored_measure = match subjects {
        [single] => DecreaseMeasure::Single(*single),
        [lower, upper] => DecreaseMeasure::Distance {
            lower: *lower,
            upper: *upper,
        },
        _ => return DecreaseOutcome::Unproven,
    };

    let states = program.machine_states(machine);
    // The ranking witness is declared on the machine signature, so its names
    // resolve against the machine's root (entry) state.
    let Some(root_state) = states.first() else {
        return DecreaseOutcome::Proven;
    };
    let decrease_order = program.machine_decrease_order(machine.ranking_witness.view);
    let view_arguments = program
        .expression_table
        .expression_handles(machine.ranking_witness.view_arguments);
    let order = match RankingOrder::resolve(
        program,
        root_state,
        subjects,
        decrease_order,
        view_arguments,
    ) {
        OrderResolution::Resolved(order) => order,
        OrderResolution::AmbiguousDefault(ambiguity) => {
            return DecreaseOutcome::AmbiguousOrder(ambiguity);
        }
        OrderResolution::Unsupported => return DecreaseOutcome::Unproven,
    };
    let measure = match (&order, authored_measure) {
        (RankingOrder::NatIncreasingTo(limit), DecreaseMeasure::Single(lower)) => {
            DecreaseMeasure::Distance {
                lower,
                upper: *limit,
            }
        }
        _ => authored_measure,
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
                    measure,
                    &order,
                    orientation,
                )
            })
    };

    if proven_with(DistanceOrientation::Declared) {
        if machine.ranking_witness.range.is_present()
            && !rank_range_proven(program, machine, measure, &order)
        {
            let range = machine.ranking_witness.range;
            return DecreaseOutcome::UnprovenRange(format!(
                "{}..{}{}",
                decreasing_value_text(program, range.start),
                if range.end_inclusive { "=" } else { "" },
                decreasing_value_text(program, range.end),
            ));
        }
        return DecreaseOutcome::Proven;
    }

    // An unproven bounded-distance tuple is probed with its subjects swapped:
    // when the swapped distance proves on every cyclic edge, the clause is the
    // named bounded distance written backwards, and the diagnostic can point
    // at the right shape.
    if let DecreaseMeasure::Distance { lower, upper } = measure
        && matches!(order, RankingOrder::BoundedDistance)
        && proven_with(DistanceOrientation::Swapped)
    {
        return DecreaseOutcome::InvertedDistance(InvertedDistance {
            declared: format!(
                "({}, {})",
                decreasing_value_text(program, lower),
                decreasing_value_text(program, upper)
            ),
            corrected: format!(
                "({}, {})",
                decreasing_value_text(program, upper),
                decreasing_value_text(program, lower)
            ),
        });
    }

    DecreaseOutcome::Unproven
}

/// Conservative discharge for an authored rank interval. Builtin natural
/// rankings have floor zero. Their obvious carrier-owned upper bound is the
/// subject itself (descending/length) or the fixed upper subject/view argument
/// (bounded distance/IncreasingTo). More involved symbolic bounds remain
/// unproven instead of being trusted.
fn rank_range_proven(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    measure: DecreaseMeasure,
    order: &RankingOrder,
) -> bool {
    let range = machine.ranking_witness.range;
    let lower_contains_zero = matches!(
        program.expression_table.expression(range.start),
        omega_typed_trees::expression::ExpressionNode::Integer(literal)
            if literal
                .value_bignum()
                .is_some_and(|value| value <= omega_core::bignum::BigInt::zero())
    );
    if !lower_contains_zero || !range.end_inclusive {
        return false;
    }

    match (order, measure) {
        (
            RankingOrder::NatDescending | RankingOrder::CustomNatDescending,
            DecreaseMeasure::Single(subject),
        ) => expressions_equivalent(program, range.end, subject),
        (
            RankingOrder::NatIncreasingTo(_) | RankingOrder::BoundedDistance,
            DecreaseMeasure::Distance { upper, .. },
        ) => expressions_equivalent(program, range.end, upper),
        (RankingOrder::SliceLength, DecreaseMeasure::Single(subject)) => {
            expression_is_length_of(program, range.end, subject)
                || expressions_equivalent(program, range.end, subject)
                    && matches!(
                        program.expression_table.expression(subject),
                        omega_typed_trees::expression::ExpressionNode::Member(member)
                            if member.member.as_str() == "len"
                    )
        }
        (RankingOrder::CustomStructView(field), DecreaseMeasure::Single(subject)) => {
            expression_is_member_of(program, range.end, subject, field.as_str())
        }
        _ => false,
    }
}

pub(super) fn machine_rank_range_proven(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> bool {
    if !machine.ranking_witness.range.is_present() {
        return true;
    }
    let Some(root_state) = program.machine_states(machine).first() else {
        return false;
    };
    let subjects = program
        .expression_table
        .expression_handles(machine.ranking_witness.subjects);
    let authored_measure = match subjects {
        [single] => DecreaseMeasure::Single(*single),
        [lower, upper] => DecreaseMeasure::Distance {
            lower: *lower,
            upper: *upper,
        },
        _ => return false,
    };
    let view = program.machine_decrease_order(machine.ranking_witness.view);
    let view_arguments = program
        .expression_table
        .expression_handles(machine.ranking_witness.view_arguments);
    let order = match RankingOrder::resolve(program, root_state, subjects, view, view_arguments) {
        OrderResolution::Resolved(order) => order,
        _ => return false,
    };
    let measure = match (&order, authored_measure) {
        (RankingOrder::NatIncreasingTo(limit), DecreaseMeasure::Single(lower)) => {
            DecreaseMeasure::Distance {
                lower,
                upper: *limit,
            }
        }
        _ => authored_measure,
    };
    rank_range_proven(program, machine, measure, &order)
}

fn expression_is_length_of(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    receiver: ExpressionHandle,
) -> bool {
    expression_is_member_of(program, expression, receiver, "len")
}

fn expression_is_member_of(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    receiver: ExpressionHandle,
    member_name: &str,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        omega_typed_trees::expression::ExpressionNode::Member(member)
            if member.member.as_str() == member_name
                && expressions_equivalent(program, member.receiver, receiver)
    )
}

fn expressions_equivalent(
    program: &omega_typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    use omega_typed_trees::expression::ExpressionNode;

    if left == right {
        return true;
    }
    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.symbol == right.symbol
                || program.expression_table.name_path_members(left.members)
                    == program.expression_table.name_path_members(right.members)
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member == right.member
                && expressions_equivalent(program, left.receiver, right.receiver)
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        _ => false,
    }
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
    measure: DecreaseMeasure,
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

    if single_state_self_loop {
        return edges.iter().all(|edge| {
            states.get(edge.from).is_some_and(|source| {
                state_has_proven_supported_self_loop(program, source, measure, order, orientation)
            })
        });
    }

    // Multi-state cycle: every edge must be STRICT or NON-INCREASING (a
    // forwarding edge passing the measure unchanged), and the subgraph of
    // non-strict edges must be ACYCLIC -- then every cycle traversal crosses
    // at least one strict decrease, which is well-founded over the naturals.
    let mut pairs: Vec<(usize, usize)> = edges.iter().map(|edge| (edge.from, edge.to)).collect();
    pairs.sort_unstable();
    pairs.dedup();
    let mut nonstrict_edges: Vec<(usize, usize)> = Vec::new();
    let all_classified = pairs.iter().all(|&(from, to)| {
        let (Some(source), Some(target)) = (states.get(from), states.get(to)) else {
            return false;
        };
        match classify_cycle_edge(program, source, target, measure, order, orientation) {
            EdgeClass::Strict => true,
            EdgeClass::NonIncreasing => {
                nonstrict_edges.push((from, to));
                true
            }
            EdgeClass::Unknown => false,
        }
    });
    all_classified && subgraph_is_acyclic(component, &nonstrict_edges)
}

pub(super) enum EdgeClass {
    Strict,
    NonIncreasing,
    Unknown,
}

pub(super) fn classify_cross_machine_edge(
    program: &omega_typed_trees::TypedTrees,
    source_machine: &omega_typed_trees::machine::Machine,
    source_state: &omega_typed_trees::state::State,
    target_state: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    arguments: &[ExpressionHandle],
) -> EdgeClass {
    let subjects = program
        .expression_table
        .expression_handles(source_machine.ranking_witness.subjects);
    let authored_measure = match subjects {
        [single] => DecreaseMeasure::Single(*single),
        [lower, upper] => DecreaseMeasure::Distance {
            lower: *lower,
            upper: *upper,
        },
        _ => return EdgeClass::Unknown,
    };
    let view = program.machine_decrease_order(source_machine.ranking_witness.view);
    let view_arguments = program
        .expression_table
        .expression_handles(source_machine.ranking_witness.view_arguments);
    let order = match RankingOrder::resolve(program, source_state, subjects, view, view_arguments) {
        OrderResolution::Resolved(order) => order,
        _ => return EdgeClass::Unknown,
    };
    let measure = match (&order, authored_measure) {
        (RankingOrder::NatIncreasingTo(limit), DecreaseMeasure::Single(lower)) => {
            DecreaseMeasure::Distance {
                lower,
                upper: *limit,
            }
        }
        _ => authored_measure,
    };
    if let (RankingOrder::Lexicographic(fields), DecreaseMeasure::Single(decreases)) =
        (&order, measure)
    {
        return lexicographic::classify_cross_machine_edge(
            program,
            source_state,
            target_state,
            arguments,
            decreases,
            fields,
        );
    }
    if let (RankingOrder::SliceLength, DecreaseMeasure::Single(decreases)) = (&order, measure) {
        return slice::classify_cross_machine_edge(
            program,
            source_state,
            target_state,
            guard,
            arguments,
            decreases,
        );
    }
    if let (RankingOrder::CustomStructView(field), DecreaseMeasure::Single(decreases)) =
        (&order, measure)
    {
        return struct_view::classify_cross_machine_edge(
            program,
            source_state,
            target_state,
            guard,
            arguments,
            decreases,
            field,
        );
    }
    if !matches!(
        order,
        RankingOrder::NatDescending
            | RankingOrder::NatIncreasingTo(_)
            | RankingOrder::BoundedDistance
            | RankingOrder::CustomNatDescending
    ) {
        return EdgeClass::Unknown;
    }
    if nat::edge_decrease_proven(
        program,
        source_state,
        target_state,
        guard,
        arguments,
        measure,
        DistanceOrientation::Declared,
    ) {
        EdgeClass::Strict
    } else if nat::edge_nonincrease_proven(program, source_state, target_state, arguments, measure)
    {
        EdgeClass::NonIncreasing
    } else {
        EdgeClass::Unknown
    }
}

/// Classify every transition statement from `source` targeting `target`:
/// the PAIR is strict only when all its statements strictly decrease; one
/// non-increasing statement makes the pair non-strict (that alternative may
/// be taken on every traversal); one unclassifiable statement fails it.
fn classify_cycle_edge(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    orientation: DistanceOrientation,
) -> EdgeClass {
    if !matches!(
        order,
        RankingOrder::NatDescending
            | RankingOrder::NatIncreasingTo(_)
            | RankingOrder::BoundedDistance
            | RankingOrder::CustomNatDescending
    ) {
        // Slice-length, struct-view and lexicographic orders stay
        // self-loop-only (no pointwise cross-state meaning).
        return EdgeClass::Unknown;
    }
    let mut class = EdgeClass::Unknown;
    for statement in program.statement_table.statements(source.statement_nodes) {
        let Some(edge) = patterns::edge_to_any_guard(program, statement, target.symbol) else {
            continue;
        };
        if nat::edge_decrease_proven(
            program,
            source,
            target,
            edge.guard,
            edge.arguments,
            measure,
            orientation,
        ) {
            if matches!(class, EdgeClass::Unknown) {
                class = EdgeClass::Strict;
            }
        } else if nat::edge_nonincrease_proven(program, source, target, edge.arguments, measure) {
            class = EdgeClass::NonIncreasing;
        } else {
            return EdgeClass::Unknown;
        }
    }
    class
}

/// DFS cycle check over the component restricted to the given edges.
fn subgraph_is_acyclic(component: &[usize], edges: &[(usize, usize)]) -> bool {
    // 0 unvisited, 1 on-stack, 2 done -- iterative coloring.
    fn visit(
        node: usize,
        edges: &[(usize, usize)],
        color: &mut std::collections::BTreeMap<usize, u8>,
    ) -> bool {
        color.insert(node, 1);
        for &(from, to) in edges {
            if from != node {
                continue;
            }
            match color.get(&to).copied().unwrap_or(0) {
                1 => return false,
                0 => {
                    if !visit(to, edges, color) {
                        return false;
                    }
                }
                _ => {}
            }
        }
        color.insert(node, 2);
        true
    }
    let mut color = std::collections::BTreeMap::new();
    component
        .iter()
        .all(|&node| color.get(&node).copied().unwrap_or(0) != 0 || visit(node, edges, &mut color))
}

fn state_has_proven_supported_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    orientation: DistanceOrientation,
) -> bool {
    // Slice-length, struct-view and lexicographic orders rank a single
    // decreasing value; only the nat provers understand the two-subject
    // bounded distance.
    match (order, measure) {
        (
            RankingOrder::NatDescending
            | RankingOrder::NatIncreasingTo(_)
            | RankingOrder::BoundedDistance
            | RankingOrder::CustomNatDescending,
            _,
        ) => nat::state_has_proven_self_loop(program, state, measure, orientation),
        (RankingOrder::SliceLength, DecreaseMeasure::Single(decreases)) => {
            slice::state_has_proven_self_loop(program, state, decreases)
        }
        (RankingOrder::CustomStructView(field), DecreaseMeasure::Single(decreases)) => {
            struct_view::state_has_proven_self_loop(program, state, decreases, field)
        }
        (RankingOrder::Lexicographic(fields), DecreaseMeasure::Single(decreases)) => {
            lexicographic::state_has_proven_self_loop(program, state, decreases, fields)
        }
        (_, DecreaseMeasure::Distance { .. }) => false,
    }
}
