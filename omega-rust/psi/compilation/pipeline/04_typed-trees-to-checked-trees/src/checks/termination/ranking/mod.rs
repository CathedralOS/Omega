//! Source ranking resolves the selected well-founded order and judges complete
//! arrivals under one coordinate assignment. The relational arithmetic owner
//! retains optional authored endpoint formation and pinning. Current scalar
//! transfer supplies writes in one exact declaration frame; mathematical and
//! lexicographic orders keep their genuine source judgments. Observation
//! metadata is projected only after source admission, not by execution.

mod current_values;
mod entry_requirements;
mod lexicographic;
mod nat;
mod observations;
mod patterns;
mod ranges;
pub(in crate::checks::termination) use ranges::RankArrivalAdmission;
mod slice;
mod struct_view;
mod write_preservation;

pub(crate) use entry_requirements::proves_ranked_entry_requirement;

use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::ranking::{
    resolve_machine_witness_subjects, resolve_machine_witness_view_arguments,
};

use super::graph;
use super::order::{AmbiguousDefault, OrderResolution, RankingOrder, decreasing_value_text};

/// The outcome of attempting to prove a machine's `terminates by`
/// clause, distinguishing the cases the caller renders as different diagnostics.
pub(super) enum DecreaseOutcome {
    /// The decrease was proven across every cyclic edge.
    Proven(Option<ranges::RankArrivalAdmission>),
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

pub(in crate::checks::termination) use observations::prepare_natural_observations;

pub(super) fn machine_decrease_outcome(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> DecreaseOutcome {
    machine_decrease_outcome_with(program, machine, call_frames, &mut |_| true)
}

pub(super) fn machine_decrease_outcome_with(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
) -> DecreaseOutcome {
    let states = program.machine_states(machine);
    // The ranking clause is declared on the machine signature, so its names
    // resolve against the machine's root (entry) state.
    let Some(root_state) = states.first() else {
        return DecreaseOutcome::Proven(None);
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
    let order = match RankingOrder::resolve_witness(
        program,
        root_state,
        witness,
        &subjects,
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

    if let Some(range) = witness.rank_range.as_ref() {
        match ranges::check(
            program,
            machine,
            range,
            &order,
            measure,
            call_frames,
            accept,
        ) {
            Ok(arrival) => return DecreaseOutcome::Proven(Some(arrival)),
            Err(message) => return DecreaseOutcome::Rejected(message),
        }
    }

    // A relational descent failure names its recurrent edge. Later provers may
    // still succeed, so it only replaces the bare refusal at the end.
    let mut blocked_edge = None;
    if witness.rank_range.is_none() {
        match ranges::admit_relational_decrease(
            program,
            machine,
            &order,
            measure,
            call_frames,
            accept,
        ) {
            Ok(arrival) => return DecreaseOutcome::Proven(Some(arrival)),
            Err(ranges::RankArrivalFailure::StrictDecrease {
                source,
                destination,
            }) => blocked_edge = Some((source, destination)),
            Err(ranges::RankArrivalFailure::RangeOrFormation) => {}
        }
    }

    if matches!(
        order,
        RankingOrder::NatDescending | RankingOrder::BoundedDistance | RankingOrder::IncreasingTo(_)
    ) && let Some(arrival) =
        current_values::admit_root_cycle(program, machine, &order, measure, call_frames)
    {
        if accept(&arrival) {
            return DecreaseOutcome::Proven(Some(arrival));
        }
    }

    // Erased structural recursion and lexicographic orders have their genuine
    // source proof vocabulary. Executable natural views instead need the one
    // complete coordinate witness above; the former spelling/shape probes
    // cannot supply a second scalar arrival route.
    let structural_order = matches!(order, RankingOrder::Lexicographic(_))
        || (!matches!(order, RankingOrder::CustomScalarView { .. })
            && validation::proof_only_classification(program).is_proof_machine(program, machine));
    if structural_order {
        let adjacency = graph::machine_adjacency(program, machine);
        let components = graph::strongly_connected_components(&adjacency);
        // Slice-tail proofs share one lazily prepared bound index. Scalar
        // admission and its diagnostics never rebuild this structural graph.
        let mut bound_lookup = None;
        if components
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
                    call_frames,
                    &mut bound_lookup,
                )
            })
        {
            return DecreaseOutcome::Proven(None);
        }
    }

    // Diagnose inversion with the same complete arrival judgment that admits
    // a scalar witness, not the structural source-pattern prover. This is a
    // proposed correction only: no swapped coordinates are retained as the
    // authored witness or supplied to call-component selection.
    if let DecreaseMeasure::Distance { lower, upper } = measure
        && matches!(order, RankingOrder::BoundedDistance)
        && ranges::admit_relational_decrease(
            program,
            machine,
            &order,
            DecreaseMeasure::Distance {
                lower: upper,
                upper: lower,
            },
            call_frames,
            &mut |_| true,
        )
        .is_ok()
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

    match blocked_edge {
        Some((source, destination)) => DecreaseOutcome::Rejected(ranges::strict_decrease_message(
            program,
            source,
            destination,
        )),
        None => DecreaseOutcome::Unproven,
    }
}

/// TPR3 slice 4: the RESOLVED view's explicit spelling for the checked
/// termination facts -- the canonical builtin path, or the authored
/// declared-measure path (the plan's recorded spelling). Empty when the
/// machine carries no witness or nothing resolves.
pub(in crate::checks::termination) fn machine_resolved_view_path(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
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
    match RankingOrder::resolve_witness(program, root_state, witness, &subjects, &view_arguments) {
        OrderResolution::Resolved(order) => {
            canonical_order_path(&order).map_or_else(|| witness.view_path.clone(), str::to_string)
        }
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
        RankingOrder::CustomNatDescending { .. }
        | RankingOrder::CustomScalarView { .. }
        | RankingOrder::CustomStructView { .. }
        | RankingOrder::Lexicographic(_) => None,
    }
}

/// A cyclic component terminates when the measure strictly decreases across
/// every transition edge that stays inside the cycle. Requiring a strict
/// decrease on each in-cycle edge is sufficient (if stronger than necessary)
/// for well-foundedness around the cycle, and it composes the existing
/// single-edge ranking proofs.
fn component_has_proven_decrease<'p>(
    program: &'p dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    adjacency: &[Vec<usize>],
    component: &[usize],
    measure: DecreaseMeasure,
    order: &RankingOrder,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
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
    // Struct views enumerate every occurrence themselves. Other custom orders
    // still have single-occurrence recognizers and must retain that restriction.
    let single_state_self_loop = component.len() == 1
        && (edges.len() == 1 || matches!(order, RankingOrder::CustomStructView { .. }));

    if single_state_self_loop {
        return edges.iter().all(|edge| {
            states.get(edge.from).is_some_and(|source| {
                state_has_provable_self_loop(
                    program,
                    machine,
                    source,
                    measure,
                    order,
                    call_frames,
                    bound_lookup,
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
        cycle_edge_strictly_decreases(program, source, target, measure, order)
    })
}

/// Every transition statement from `source` targeting `target` must strictly
/// decrease the rank. One unclassifiable or merely forwarding alternative
/// rejects the pair because that alternative may be taken on every traversal.
fn cycle_edge_strictly_decreases(
    program: &dyn ::validation::CheckingInput,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    measure: DecreaseMeasure,
    order: &RankingOrder,
) -> bool {
    if !matches!(
        order,
        RankingOrder::NatDescending
            | RankingOrder::BoundedDistance
            | RankingOrder::IncreasingTo(_)
            | RankingOrder::CustomNatDescending { .. }
    ) {
        // Slice-length, struct-view and lexicographic orders stay
        // self-loop-only (no pointwise cross-state meaning). Computed views
        // require the produced-rank judgment above; their inputs alone carry
        // no monotonicity guarantee.
        return false;
    }
    let mut found = false;
    for edge in patterns::edges_to_state(program, source, target.symbol) {
        if nat::edge_decrease_proven(
            program,
            source,
            target,
            &edge.guards,
            edge.arguments,
            measure,
        ) {
            found = true;
        } else {
            return false;
        }
    }
    found
}

fn state_has_provable_self_loop<'p>(
    program: &'p dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> bool {
    // Slice-length, struct-view and lexicographic orders rank a single
    // decreasing value; a two-subject distance belongs to `current_values`.
    match (order, measure) {
        // A declared computation needs its produced-value proof, including
        // formation. Structural/raw-subject fallback cannot supply it.
        (RankingOrder::CustomScalarView { .. }, _) => false,
        (
            RankingOrder::NatDescending
            | RankingOrder::BoundedDistance
            | RankingOrder::IncreasingTo(_)
            | RankingOrder::CustomNatDescending { .. },
            _,
        ) => nat::state_has_proven_self_loop(program, state, measure),
        (RankingOrder::SliceLength, DecreaseMeasure::Single(decreases)) => {
            slice::state_has_proven_self_loop(program, machine, state, decreases, bound_lookup)
        }
        (
            RankingOrder::CustomStructView {
                path,
                field,
                field_symbol,
                owner,
                ..
            },
            DecreaseMeasure::Single(decreases),
        ) => struct_view::state_has_proven_self_loop(
            program,
            machine,
            state,
            decreases,
            path,
            field,
            *field_symbol,
            *owner,
            call_frames,
        ),
        (RankingOrder::Lexicographic(fields), DecreaseMeasure::Single(decreases)) => {
            lexicographic::state_has_proven_self_loop(program, state, decreases, fields)
        }
        (_, DecreaseMeasure::Distance { .. }) => false,
    }
}
