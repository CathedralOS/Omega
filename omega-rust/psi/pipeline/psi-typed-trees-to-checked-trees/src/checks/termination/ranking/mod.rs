mod lexicographic;
mod nat;
mod patterns;
mod slice;
mod struct_view;

use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::ranking::{
    resolve_machine_witness_subjects, resolve_machine_witness_view_arguments,
};

use super::graph;
use super::order::{AmbiguousDefault, OrderResolution, RankingOrder, decreasing_value_text};

/// The outcome of attempting to prove a machine's `terminates by`
/// clause, distinguishing the cases the caller renders as different diagnostics.
pub(super) enum DecreaseOutcome {
    /// The decrease was proven across every cyclic edge.
    Proven,
    /// The order resolved, but the decrease could not be proven.
    Unproven,
    /// A plain `terminates by value` clause whose decreasing value has no single
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
/// two-subject ranking tuple: the declared `(lower, upper)`, or the swapped
/// probe used to recognize an inverted bounded distance.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DistanceOrientation {
    Declared,
    Swapped,
}

/// The ranked subjects a `terminates by` clause threads into the edge provers: a
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

/// Exact first-slice evidence exported to checked structural-control
/// construction. The ranking checker remains the sole recognizer; consumers
/// receive resolved state/parameter/edge coordinates only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProvenNatCountdownScc {
    pub(crate) header_state: psi_symbols::SymbolHandle,
    pub(crate) header_rank_parameter_position: u32,
    pub(crate) rank_primitive_type: psi_typed_trees::types::PrimitiveType,
    pub(crate) rank_lower_bound: u128,
    pub(crate) rank_upper_bound: u128,
    pub(crate) covered_cyclic_edges: Vec<ProvenNatCountdownEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProvenNatCountdownEdge {
    pub(crate) source_state: psi_symbols::SymbolHandle,
    pub(crate) target_state: psi_symbols::SymbolHandle,
    pub(crate) statement_ordinal: u32,
    pub(crate) source_rank_parameter_position: u32,
    pub(crate) target_rank_parameter_position: u32,
}

/// Project the existing Nat-descending judgment into exact first-slice SCC
/// evidence. `Some([])` means acyclic. `None` means a cycle exists but is not
/// the single-state, directly guarded unsigned countdown admitted by this
/// checked-plan milestone.
pub(crate) fn proven_nat_countdown_sccs(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<Vec<ProvenNatCountdownScc>> {
    let adjacency = graph::machine_adjacency(program, machine);
    let cyclic_components = graph::strongly_connected_components(&adjacency)
        .into_iter()
        .filter(|component| graph::component_is_cyclic(&adjacency, component))
        .collect::<Vec<_>>();
    if cyclic_components.is_empty() {
        return Some(Vec::new());
    }
    if !matches!(
        machine_decrease_outcome(program, machine),
        DecreaseOutcome::Proven
    ) {
        return None;
    }

    let states = program.machine_states(machine);
    let root_state = states.first()?;
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    let subjects = resolve_machine_witness_subjects(program, machine)?;
    let [decreases] = subjects.as_slice() else {
        return None;
    };
    let ExpressionNode::Name(decreases_path) = program.expression_table.expression(*decreases)
    else {
        return None;
    };
    let ranking_view = witness
        .view_path
        .split("::")
        .filter(|member| !member.is_empty())
        .collect::<Vec<_>>();
    let OrderResolution::Resolved(RankingOrder::NatDescending) =
        RankingOrder::resolve(program, root_state, &[*decreases], &ranking_view, &[])
    else {
        return None;
    };

    let mut retained = Vec::with_capacity(cyclic_components.len());
    for component in cyclic_components {
        let [state_index] = component.as_slice() else {
            return None;
        };
        let state = states.get(*state_index)?;
        let decrease_name = program
            .expression_table
            .name_path_members(decreases_path.members)
            .last()
            .map(|member| member.as_str())
            .unwrap_or_default();
        let (header_rank_parameter_position, rank_parameter) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| {
                !parameter.is_self
                    && (parameter.symbol == decreases_path.symbol
                        || parameter.name.as_str() == decrease_name)
            })?;
        let rank_primitive_type =
            program.primitive_type_reference(rank_parameter.type_reference)?;
        let rank_upper_bound = unsigned_maximum(rank_primitive_type)?;

        let mut covered_cyclic_edges = Vec::new();
        for (statement_ordinal, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let Some(edge) = patterns::edge_to_any_guard(program, statement, state.symbol) else {
                continue;
            };
            let proof = nat::direct_countdown_edge(
                program,
                state,
                state,
                edge.guard,
                edge.arguments,
                *decreases,
            )?;
            if proof.source_parameter != rank_parameter.symbol {
                return None;
            }
            let target_parameter = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .nth(proof.target_argument_index)?;
            let target_rank_parameter_position = program
                .state_parameters(state)
                .iter()
                .position(|parameter| parameter.symbol == target_parameter.symbol)?;
            if target_parameter.symbol != rank_parameter.symbol {
                return None;
            }
            covered_cyclic_edges.push(ProvenNatCountdownEdge {
                source_state: state.symbol,
                target_state: state.symbol,
                statement_ordinal: u32::try_from(statement_ordinal).ok()?,
                source_rank_parameter_position: u32::try_from(header_rank_parameter_position)
                    .ok()?,
                target_rank_parameter_position: u32::try_from(target_rank_parameter_position)
                    .ok()?,
            });
        }
        if covered_cyclic_edges.is_empty() {
            return None;
        }
        retained.push(ProvenNatCountdownScc {
            header_state: state.symbol,
            header_rank_parameter_position: u32::try_from(header_rank_parameter_position).ok()?,
            rank_primitive_type,
            rank_lower_bound: 0,
            rank_upper_bound,
            covered_cyclic_edges,
        });
    }
    Some(retained)
}

fn unsigned_maximum(primitive: psi_typed_trees::types::PrimitiveType) -> Option<u128> {
    use psi_typed_trees::types::PrimitiveType;
    match primitive {
        PrimitiveType::U8 => Some(u128::from(u8::MAX)),
        PrimitiveType::U16 => Some(u128::from(u16::MAX)),
        PrimitiveType::U32 => Some(u128::from(u32::MAX)),
        PrimitiveType::U64 => Some(u128::from(u64::MAX)),
        PrimitiveType::Bool
        | PrimitiveType::F32
        | PrimitiveType::F64
        | PrimitiveType::I8
        | PrimitiveType::I16
        | PrimitiveType::I32
        | PrimitiveType::I64
        | PrimitiveType::Addr => None,
    }
}

pub(super) fn machine_decrease_outcome(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> DecreaseOutcome {
    let states = program.machine_states(machine);
    // The ranking clause is declared on the machine signature, so its names
    // resolve against the machine's root (entry) state.
    let Some(root_state) = states.first() else {
        return DecreaseOutcome::Proven;
    };
    let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
        return DecreaseOutcome::Unproven;
    };
    let Some(subjects) = resolve_machine_witness_subjects(program, machine) else {
        return DecreaseOutcome::Rejected(
            "internal: normalized ranking-witness subjects did not resolve in the root state"
                .to_string(),
        );
    };
    let Some(view_arguments) = resolve_machine_witness_view_arguments(program, machine) else {
        return DecreaseOutcome::Rejected(
            "internal: normalized ranking-view arguments did not resolve in the root state"
                .to_string(),
        );
    };
    let ranking_view = witness
        .view_path
        .split("::")
        .filter(|member| !member.is_empty())
        .collect::<Vec<_>>();
    let order = match RankingOrder::resolve(
        program,
        root_state,
        &subjects,
        &ranking_view,
        &view_arguments,
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
    let measure = match (&order, subjects.as_slice()) {
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
    if witness.ranking_view.is_valid()
        && let Some(recorded_path) = witness.ranking_view.canonical_path()
        && let Some(resolved_path) = canonical_order_path(&order)
        && recorded_path != resolved_path
    {
        return DecreaseOutcome::PlanViewDivergence {
            recorded: recorded_path.to_string(),
            resolved: resolved_path.to_string(),
        };
    }

    // TPR3 slice 3: the `in <range>` rank constraint is CONSUMED here. V1
    // verifies the structurally-true shape and rejects everything else with
    // a directed message -- never a silent drop, never an unproven fact.
    if let Some(range) = witness.rank_range.as_ref()
        && let Some(message) = rank_range_violation(program, range, &order)
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
    program: &psi_typed_trees::TypedTrees,
    range: &psi_language_semantics::RankRange,
    order: &RankingOrder,
) -> Option<String> {
    if range.floor != "0" {
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
            if !range.ceiling_inclusive {
                return Some(format!(
                    "the rank of `Nat::IncreasingTo({bound})` reaches `{bound}` itself -- \
                     spell the ceiling inclusively (`in 0..={bound}`)"
                ));
            }
            (range.ceiling != bound).then(|| {
                format!(
                    "the rank ceiling `{}` is not the view's own bound `{bound}`: \
                     only `in 0..={bound}` verifies structurally on \
                     `Nat::IncreasingTo({bound})` today (decision 23 TPR3)",
                    range.ceiling
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
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> String {
    let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
        return String::new();
    };
    let Some(root_state) = program.machine_states(machine).first() else {
        return String::new();
    };
    let Some(subjects) = resolve_machine_witness_subjects(program, machine) else {
        return String::new();
    };
    let Some(view_arguments) = resolve_machine_witness_view_arguments(program, machine) else {
        return String::new();
    };
    let ranking_view = witness
        .view_path
        .split("::")
        .filter(|member| !member.is_empty())
        .collect::<Vec<_>>();
    match RankingOrder::resolve(
        program,
        root_state,
        &subjects,
        &ranking_view,
        &view_arguments,
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
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
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
                state_has_proven_supported_self_loop(
                    program,
                    machine,
                    source,
                    measure,
                    order,
                    orientation,
                )
            })
        });
    }

    // Frozen decision 23 deliberately uses the simple compositional rule:
    // EVERY edge that stays inside a cyclic component must strictly decrease
    // the selected rank. A merely forwarding edge is rejected even when some
    // later edge decreases; accepting it would make admissibility depend on a
    // second graph-level algebra that the public ranking law does not expose.
    let mut pairs: Vec<(usize, usize)> = edges.iter().map(|edge| (edge.from, edge.to)).collect();
    pairs.sort_unstable();
    pairs.dedup();
    pairs.iter().all(|&(from, to)| {
        let (Some(source), Some(target)) = (states.get(from), states.get(to)) else {
            return false;
        };
        cycle_edge_strictly_decreases(program, source, target, measure, order, orientation)
    })
}

/// Every transition statement from `source` targeting `target` must strictly
/// decrease the rank. One unclassifiable or merely forwarding alternative
/// rejects the pair because that alternative may be taken on every traversal.
fn cycle_edge_strictly_decreases(
    program: &psi_typed_trees::TypedTrees,
    source: &psi_typed_trees::state::State,
    target: &psi_typed_trees::state::State,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    orientation: DistanceOrientation,
) -> bool {
    if !matches!(
        order,
        RankingOrder::NatDescending
            | RankingOrder::BoundedDistance
            | RankingOrder::IncreasingTo(_)
            | RankingOrder::CustomNatDescending
    ) {
        // Slice-length, struct-view and lexicographic orders stay
        // self-loop-only (no pointwise cross-state meaning).
        return false;
    }
    let mut found = false;
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
            found = true;
        } else {
            return false;
        }
    }
    found
}

fn state_has_proven_supported_self_loop(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
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
            slice::state_has_proven_self_loop(program, machine, state, decreases)
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
