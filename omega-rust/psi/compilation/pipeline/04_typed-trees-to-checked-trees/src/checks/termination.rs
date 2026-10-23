//! Source termination admission and its retained checked decisions.
//!
//! `admit_machine_rankings` runs once after completed effects and caller frames.
//! Local complete candidates stay in one lazy search until required runtime
//! call edges and every-cycle descent accept the same coordinates. It then
//! retains the exact body judgments and complete component decision once.
//! Progress reads those decisions while independently propagating other calls'
//! selected interface promises. Final source checking reports the retained
//! diagnostics; late value computation and execution selection only project
//! accepted observations. They never select another carrier or replay descent.
//!
//! Package semantic admission has its own pre-check invocation of the same
//! source judgment; it cannot consume a later compile's retained facts. Proof
//! recursion and runtime call topology remain independent early authorities.

mod component_selection;
mod graph;
mod order;
mod progress;
mod ranking;
mod source_natural;

pub(crate) use progress::{entry_covers_progress_premises, profile_requirement_covered};

// Contract induction uses the same authored-edge graph as ranking; graph
// membership supplies no ranking or invariant evidence by itself.
pub(crate) use graph::{machine_adjacency, strongly_connected_components};
pub(crate) use ranking::proves_ranked_entry_requirement;

use crate::labels::machine_name;
use diagnostics::Diagnostic;

/// Admit source ranking once after independent effects and caller frames are
/// complete. All exact authored bodies retain their local result; a selected
/// relational witness additionally retains its coherent full coordinates.
pub(crate) fn admit_machine_rankings(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> checked_trees::CheckedRankArrivalPlan {
    let mut admitted = checked_trees::CheckedRankArrivalPlan::default();
    let topology = validation::prepare_runtime_rank_components(program);
    let mut components = topology
        .as_ref()
        .map(|topology| {
            topology
                .joint_components(program)
                .map(|component| {
                    component_selection::select(program, topology, component, call_frames)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for (position, machine) in program.machines().iter().enumerate() {
        let cycle = graph::machine_has_cycle(program, machine);
        let witness = machine.termination_plan.implementation_witness.as_ref();
        let claimed = witness.is_some()
            || matches!(
                &machine.termination_plan.interface,
                language_semantics::TerminationInterface::Published(
                    language_semantics::TerminationGuarantee::Terminates { .. }
                )
            );
        // A locally acyclic prefix can belong to a recurrent call component.
        // Only a demanded arrival witness adds that obligation: an unused
        // unbounded view cannot reject an ordinary acyclic body.
        let needs_judgment = witness.is_some_and(|witness| {
            cycle
                || witness.rank_range.is_some()
                || topology
                    .as_ref()
                    .is_some_and(|topology| topology.requires_arrival_witness(position))
        });
        let mut local_terminates =
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody && !cycle;
        let mut entry_rank_formed = false;
        let mut diagnostic = String::new();
        if needs_judgment {
            let component = components
                .iter_mut()
                .find(|component| component.positions.contains(&position));
            let outcome = if let Some(component) = component {
                if let Some(body) = component
                    .bodies
                    .iter()
                    .position(|(member, _)| *member == position)
                {
                    Some(ranking::DecreaseOutcome::Proven(Some(
                        component.bodies.remove(body).1,
                    )))
                } else {
                    component
                        .source_outcomes
                        .iter()
                        .position(|(member, _)| *member == position)
                        .map(|failure| component.source_outcomes.remove(failure).1)
                }
            } else {
                Some(ranking::machine_decrease_outcome(
                    program,
                    machine,
                    call_frames,
                ))
            };
            match outcome {
                None => {
                    local_terminates = false;
                }

                Some(ranking::DecreaseOutcome::Proven(arrival)) => {
                    entry_rank_formed = witness.is_some_and(|witness| witness.rank_range.is_some());
                    local_terminates =
                        machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody;
                    if let Some(arrival) = arrival
                        && arrival.retain(program, &mut admitted, machine).is_none()
                    {
                        local_terminates = false;
                        entry_rank_formed = false;
                        diagnostic = format!(
                            "internal: cannot retain ranked arrival witness for machine {}",
                            machine_name(program, machine.symbol)
                        );
                    }
                }
                Some(ranking::DecreaseOutcome::Unproven) => {
                    local_terminates = false;
                    diagnostic = format!(
                        "cannot prove the `terminates by` ranking for machine {}",
                        machine_name(program, machine.symbol)
                    );
                }
                Some(ranking::DecreaseOutcome::AmbiguousOrder(ambiguity)) => {
                    local_terminates = false;
                    diagnostic =
                        ambiguous_order_message(&machine_name(program, machine.symbol), &ambiguity);
                }
                Some(ranking::DecreaseOutcome::InvertedDistance(inverted)) => {
                    local_terminates = false;
                    diagnostic = inverted_distance_message(
                        &machine_name(program, machine.symbol),
                        &inverted,
                    );
                }
                Some(ranking::DecreaseOutcome::Rejected(message)) => {
                    local_terminates = false;
                    diagnostic = format!(
                        "terminating machine {}: {message}",
                        machine_name(program, machine.symbol)
                    );
                }
            }
        } else if claimed
            && (cycle
                || topology
                    .as_ref()
                    .is_some_and(|topology| topology.requires_arrival_witness(position)))
            && witness.is_none()
        {
            diagnostic = format!(
                "terminating machine {} contains a recursive cycle but carries no \
                 ranking witness -- spell one with `terminates by <subjects> [-> View];` \
                 (decision 23)",
                machine_name(program, machine.symbol)
            );
        }
        if local_terminates
            && witness.is_some()
            && admitted.for_body(machine.symbol, machine.states).is_none()
        {
            if let Some(identity) = ranking::RankArrivalAdmission::entry_only(program, machine) {
                if identity.retain(program, &mut admitted, machine).is_none() {
                    local_terminates = false;
                    entry_rank_formed = false;
                    diagnostic = format!(
                        "internal: cannot retain initial rank coordinate for machine {}",
                        machine_name(program, machine.symbol),
                    );
                }
            }
        }
        let observations = if local_terminates && witness.is_some() {
            ranking::prepare_natural_observations(program, machine, &admitted)
                .map(|observations| admitted.observations.insert_many(observations))
                .unwrap_or_default()
        } else {
            Default::default()
        };
        admitted
            .judgments
            .insert(checked_trees::CheckedRankBodyJudgment {
                machine: machine.symbol,
                body: machine.states,
                local_terminates,
                entry_rank_formed,
                observations,
                diagnostic,
            });
    }
    for component in components {
        if let Some(source) = component.source_natural {
            let members = admitted.source_natural_members.insert_many(source.members);
            let edges = admitted.source_natural_edges.insert_many(source.edges);
            admitted.source_natural_components.insert(
                checked_trees::CheckedSourceNaturalRankComponent {
                    members,
                    edges,
                    view: language_semantics::RankingViewId::NAT_DESCENDING,
                },
            );
            continue;
        }
        validation::retain_runtime_rank_component(
            program,
            &component.positions,
            component.decision,
            &mut admitted,
        );
    }
    admitted
}

pub(crate) fn check_machine_termination(
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = admitted
        .judgments
        .iter()
        .filter_map(|(_, judgment)| {
            (!judgment.diagnostic.is_empty())
                .then(|| Diagnostic::error(judgment.diagnostic.clone()))
        })
        .chain(admitted.runtime_components.iter().filter_map(|(_, row)| {
            (!row.diagnostic.is_empty()).then(|| Diagnostic::error(row.diagnostic.clone()))
        }))
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The early indexed logical profile establishes proof placement and absence
/// of operational effects. It does not establish that the source body reaches
/// a terminal outcome: `ensures` is partial correctness. Every machine admitted
/// through that proof-side profile therefore needs the independently retained
/// whole-machine progress judgment before execution can erase its citations or
/// omit its erased control. A written ranking or an empty diagnostic is not
/// the positive checked totality result.
pub(crate) fn check_indexed_logical_totality(
    program: &dyn ::validation::CheckingInput,
    termination: &checked_trees::TerminationFacts,
) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = program.machines().iter().filter(|machine| {
        validation::mathematical_applications::is_indexed_logical_machine(program, machine)
    }).filter_map(|machine| {
        let unconditional = termination.for_machine(machine.symbol).is_some_and(|plan| {
            matches!(&plan.checked_summary,
                language_semantics::TerminationGuarantee::Terminates { premises } if premises.is_empty())
        });
        (!unconditional).then(|| Diagnostic::error(format!(
            "indexed logical machine `{}` has no independently checked unconditional termination; a proved invariant cannot erase a potentially nonterminating source body",
            machine_name(program, machine.symbol),
        )))
    }).collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Progress inference consumes the already admitted exact body's local
/// result. Public promise selection remains independently owned by the call
/// summary query; a private body result cannot strengthen a public interface.
pub(crate) fn retained_machine_local_summary(
    machine: &typed_trees::machine::Machine,
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> language_semantics::TerminationGuarantee {
    if admitted
        .judgment_for_body(machine.symbol, machine.states)
        .is_some_and(|judgment| judgment.local_terminates)
    {
        language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    } else {
        language_semantics::TerminationGuarantee::NoGuarantee
    }
}

pub(crate) fn build_checked_termination_plan(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    checked_summary: language_semantics::TerminationGuarantee,
) -> language_semantics::MachineTerminationPlan {
    let mut plan = machine.termination_plan.clone();
    plan.checked_summary = checked_summary;
    if let Some(witness) = plan.implementation_witness.as_mut() {
        witness.view_path = ranking::machine_resolved_view_path(program, machine);
    }
    plan
}

pub(crate) use progress::analyze_checked_progress;

pub(crate) fn infer_machine_checked_summary(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> language_semantics::TerminationGuarantee {
    use language_semantics::TerminationGuarantee;

    let established = if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        false
    } else if !graph::machine_has_cycle(program, machine) {
        true
    } else if machine.termination_plan.implementation_witness.is_some() {
        matches!(
            ranking::machine_decrease_outcome(program, machine, call_frames),
            ranking::DecreaseOutcome::Proven(_)
        )
    } else {
        false
    };

    if established {
        TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    } else {
        TerminationGuarantee::NoGuarantee
    }
}

/// Render the diagnostic for a plain `terminates by value` clause whose value has
/// no inferable builtin ranking: name the value, say why inference failed, and
/// suggest the explicit `-> View` form. Declared measures matching the value's
/// type are suggested by name but are never selected implicitly — even a single
/// declared measure requires the explicit form, so declaring a second measure
/// later cannot silently change distant `terminates by` clauses.
fn ambiguous_order_message(machine: &str, ambiguity: &order::AmbiguousDefault) -> String {
    let clause = ambiguity.clause.as_str();
    let reason = match &ambiguity.reason {
        order::AmbiguityReason::SignedInteger => {
            "signed values have no default well-founded order".to_string()
        }
        order::AmbiguityReason::NoBuiltinOrder { type_name } => {
            format!("`{type_name}` has no builtin well-founded order")
        }
        order::AmbiguityReason::UnknownShape => {
            "the decreasing value has no single builtin well-founded order".to_string()
        }
    };
    let suggestion = match ambiguity.declared_measures.as_slice() {
        [] => format!(
            "select one with `terminates by {clause} -> View` \
             (builtin views: Nat::Descending, Nat::BoundedDistance, Slice::Length)"
        ),
        [only] => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> {only}`"
        ),
        many => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> View` (declared measures: {})",
            many.join(", ")
        ),
    };
    format!(
        "cannot infer a ranking for `terminates by {clause}` in machine {machine}: \
         {reason} -- {suggestion}"
    )
}

/// Render the diagnostic for a two-subject `terminates by` tuple whose subjects
/// are inverted: the swapped subjects prove as the named bounded distance, so
/// the message names the ranking and the corrected spelling instead of a bare
/// "cannot prove".
fn inverted_distance_message(machine: &str, inverted: &ranking::InvertedDistance) -> String {
    let declared = inverted.declared.as_str();
    let corrected = inverted.corrected.as_str();
    format!(
        "cannot prove the `terminates by` ranking for machine {machine}: \
         `terminates by {declared}` inverts the named bounded distance -- \
         `Nat::BoundedDistance` ranks `(lower, upper)`, which descends as the \
         lower value climbs; write `terminates by {corrected} -> Nat::BoundedDistance`"
    )
}
