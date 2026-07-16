mod lexicographic;
mod nat;
mod patterns;
mod slice;
mod struct_view;

use omega_typed_trees::expression::ExpressionHandle;

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
    /// A two-subject bounded distance written backwards: the declared
    /// `(upper, lower)` tuple cannot decrease, but the swapped subjects prove
    /// as the named bounded distance on every cyclic edge. The diagnostic
    /// names the right shape instead of a bare "cannot prove".
    InvertedDistance(InvertedDistance),
    /// TPR3 slice 1 (decision 23): the plan's RECORDED witness view (the
    /// canonical-default elaboration at the syntax->resolved lowering) and
    /// the checker's independently resolved order DISAGREE. This is an
    /// internal invariant, never a user error -- the lowering mirrors the
    /// checker's inference exactly, so a divergence means one of them
    /// changed without the other.
    PlanViewDivergence { recorded: String, resolved: String },
    /// TPR3: a DIRECTED rejection from view resolution (unbounded
    /// increasing view, argument-arity mismatch, arguments on a plain
    /// view); the message names the fix and is rendered verbatim.
    Rejected(String),
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

/// The ranked subjects a `decreases` clause threads into the edge provers: a
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
        .expression_handles(machine.decreases);
    let states = program.machine_states(machine);
    // The decreases clause is declared on the machine signature, so its names
    // resolve against the machine's root (entry) state.
    let Some(root_state) = states.first() else {
        return DecreaseOutcome::Proven;
    };
    let decrease_order = program.machine_decrease_order(machine.decrease_order);
    let view_arguments = program
        .expression_table
        .expression_handles(machine.decrease_view_arguments);
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
        OrderResolution::Rejected { message } => return DecreaseOutcome::Rejected(message),
    };

    // The measure follows the resolved view: `Nat::IncreasingTo(limit)`
    // ranks its single subject by the distance up to the named bound (the
    // bounded-distance machinery with a view-fixed orientation); everything
    // else ranks the subjects directly.
    let measure = match (&order, subjects) {
        (RankingOrder::IncreasingTo(limit), [single]) => DecreaseMeasure::Distance {
            lower: *single,
            upper: *limit,
        },
        (_, [single]) => DecreaseMeasure::Single(*single),
        (_, [lower, upper]) => DecreaseMeasure::Distance {
            lower: *lower,
            upper: *upper,
        },
        _ => return DecreaseOutcome::Unproven,
    };

    // TPR3 slice 1 (decision 23): when the plan RECORDED an elaborated view
    // (TPR2's lowering-time canonical defaults / authored builtins) and the
    // checker's resolution lands on a canonical builtin, the two must agree
    // -- the recorded witness is what proof-cache keys and diagnostics will
    // trust, so a silent divergence is unacceptable. Declared-measure orders
    // resolve from the same authored path the plan recorded (agreement by
    // construction), and a PENDING plan view (empty path) constrains
    // nothing.
    if let Some(witness) = machine.termination_plan.implementation_witness.as_ref()
        && !witness.view_path.is_empty()
        && let Some(resolved_path) = canonical_order_path(&order)
        && witness.view_path != resolved_path
    {
        return DecreaseOutcome::PlanViewDivergence {
            recorded: witness.view_path.clone(),
            resolved: resolved_path.to_string(),
        };
    }

    // TPR3 slice 3: the `in <range>` rank constraint is CONSUMED here. V1
    // verifies the structurally-true shape and rejects everything else with
    // a directed message -- never a silent drop, never an unproven fact.
    if machine.decrease_range.is_valid()
        && let Some(message) = rank_range_violation(program, machine, &order)
    {
        return DecreaseOutcome::Rejected(message);
    }

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

/// TPR3 slice 3: verify the authored `in <range>` rank constraint against
/// the resolved view. V1 accepts exactly the shape that is TRUE BY THE
/// VIEW'S DEFINITION -- floor `0` (every builtin view produces a natural
/// rank) and a ceiling equal to the argumented view's own bound, spelled
/// identically and inclusively (`in 0..=limit` on `Nat::IncreasingTo(limit)`:
/// the rank IS the distance up to that bound). Everything else needs an
/// invariant proof that does not exist yet and is rejected with a directed
/// message.
fn rank_range_violation(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    order: &RankingOrder,
) -> Option<String> {
    use omega_typed_trees::expression::ExpressionNode;

    let ExpressionNode::Range(range) = program.expression_table.expression(machine.decrease_range)
    else {
        return Some(
            "internal: the rank-range constraint did not lower to a Range expression".to_string(),
        );
    };
    let floor_is_zero = matches!(
        program.expression_table.expression(range.start),
        ExpressionNode::Integer(literal) if literal.text() == "0"
    );
    if !floor_is_zero {
        return Some(
            "a rank floor above the natural floor `0` is not consumed yet (decision 23 \
             TPR3): every builtin view produces a natural rank -- spell `in 0..=<bound>` \
             or omit the range"
                .to_string(),
        );
    }
    match order {
        RankingOrder::IncreasingTo(limit) => {
            let bound = decreasing_value_text(program, *limit);
            if !range.end_inclusive {
                return Some(format!(
                    "the rank of `Nat::IncreasingTo({bound})` reaches `{bound}` itself -- \
                     spell the ceiling inclusively (`in 0..={bound}`)"
                ));
            }
            let ceiling = decreasing_value_text(program, range.end);
            (ceiling != bound).then(|| {
                format!(
                    "the rank ceiling `{ceiling}` is not the view's own bound `{bound}`: \
                     only `in 0..={bound}` verifies structurally on \
                     `Nat::IncreasingTo({bound})` today (decision 23 TPR3)"
                )
            })
        }
        _ => Some(
            "a rank range is only consumed on the argumented `Nat::IncreasingTo(bound)` \
             today (`in 0..=bound`, decision 23 TPR3) -- other views' ceilings need \
             invariant proofs that do not exist yet"
                .to_string(),
        ),
    }
}

/// TPR3 slice 4: the RESOLVED view's explicit spelling for the checked
/// termination facts -- the canonical builtin path, or the authored
/// declared-measure path (the plan's recorded spelling). Empty when the
/// machine carries no witness or nothing resolves.
pub(in crate::checks::termination) fn machine_resolved_view_path(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> String {
    let subjects = program
        .expression_table
        .expression_handles(machine.decreases);
    if subjects.is_empty() {
        return String::new();
    }
    let Some(root_state) = program.machine_states(machine).first() else {
        return String::new();
    };
    let decrease_order = program.machine_decrease_order(machine.decrease_order);
    let view_arguments = program
        .expression_table
        .expression_handles(machine.decrease_view_arguments);
    match RankingOrder::resolve(
        program,
        root_state,
        subjects,
        decrease_order,
        view_arguments,
    ) {
        OrderResolution::Resolved(order) => canonical_order_path(&order)
            .map(str::to_string)
            .unwrap_or_else(|| {
                machine
                    .termination_plan
                    .implementation_witness
                    .as_ref()
                    .map(|witness| witness.view_path.clone())
                    .unwrap_or_default()
            }),
        _ => String::new(),
    }
}

/// The canonical spelling of a BUILTIN resolved order (`None` for declared
/// measures, whose normalized identity lands with the rest of TPR3).
fn canonical_order_path(order: &RankingOrder) -> Option<&'static str> {
    match order {
        RankingOrder::NatDescending => Some("Nat::Descending"),
        RankingOrder::BoundedDistance => Some("Nat::BoundedDistance"),
        RankingOrder::SliceLength => Some("Slice::Length"),
        RankingOrder::IncreasingTo(_) => Some("Nat::IncreasingTo"),
        RankingOrder::CustomNatDescending
        | RankingOrder::CustomStructView(_)
        | RankingOrder::Lexicographic(_) => None,
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

enum EdgeClass {
    Strict,
    NonIncreasing,
    Unknown,
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
            | RankingOrder::BoundedDistance
            | RankingOrder::IncreasingTo(_)
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
    fn visit(node: usize, edges: &[(usize, usize)], color: &mut std::collections::BTreeMap<usize, u8>) -> bool {
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

/// Prove a strict decrease across one cyclic edge between (possibly distinct)
/// states. Only orders with a pointwise cross-state meaning are accepted here.
fn edge_has_proven_decrease(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    orientation: DistanceOrientation,
) -> bool {
    match order {
        RankingOrder::NatDescending
        | RankingOrder::BoundedDistance
        | RankingOrder::IncreasingTo(_)
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
                    measure,
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
            | RankingOrder::BoundedDistance
            | RankingOrder::IncreasingTo(_)
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
