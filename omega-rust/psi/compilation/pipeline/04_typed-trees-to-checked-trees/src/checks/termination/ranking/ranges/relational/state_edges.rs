//! Select one exact coordinate assignment after proving every complete arrival.
//! Candidate lineage does not establish equality or edge coverage. Constraints
//! prune a partial assignment only when the relevant edge inputs are complete.

use super::super::super::{graph, patterns};
use super::{RankArrivalFailure, preserved_entry_prefix, protected_input_paths};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;

pub(super) struct SelectedCorrespondence {
    pub states: Vec<validation::RankStateBindings>,
    pub origins: Vec<validation::StateEntryCorrespondence>,
    pub edges: Vec<checked_trees::CheckedRankArrivalEdge>,
}

pub(super) fn select<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program Machine,
    range: ExpressionHandle,
    measure: validation::RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'program>>,
    premises: validation::RankingRangePremises,
    premise_inputs: &[SymbolHandle],
    root_edges: &[checked_trees::CheckedRankArrivalEdge],
    accept: &mut dyn FnMut(&super::RankArrivalAdmission) -> bool,
    failure: &mut RankArrivalFailure,
) -> Option<SelectedCorrespondence> {
    let states = program.machine_states(machine);
    let mut adjacency = graph::machine_adjacency(program, machine);
    for targets in &mut adjacency {
        targets.sort_unstable();
        targets.dedup();
    }
    let entry_is_initial = !adjacency.iter().any(|targets| targets.contains(&0));
    let discovered = validation::discover_rank_arrival_correspondence(program, machine);
    let Some(discovered) = discovered else { eprintln!("Z41: discover_rank_arrival_correspondence None"); return None; };
    let required =
        validation::ranking_range_required_coordinates(program, machine, range, measure, premises);
    let Some(required) = required else { eprintln!("Z41: required_coordinates None"); return None; };
    let relevance = validation::rank_relevance(program, &[machine.symbol]);
    let Some(relevance) = relevance else { eprintln!("Z41: rank_relevance None"); return None; };
    let demands = states
        .iter()
        .filter(|state| relevance.requires_rank(state.symbol))
        .flat_map(|state| {
            required
                .iter()
                .map(move |entry| validation::RankArrivalDemand {
                    state: state.symbol,
                    entry: entry.clone(),
                })
        })
        .collect::<Vec<_>>();
    let components = graph::strongly_connected_components(&adjacency);
    let mut accepted_edges = Vec::new();
    let states = validation::select_rank_arrival_correspondence(
        program,
        machine,
        &discovered,
        &demands,
        |selected| {
            let mut complete = true;
            let mut edge_judgments = Vec::new();
            for (source_position, source) in states.iter().enumerate() {
                let Some(source_bindings) = selected
                    .iter()
                    .find(|selection| selection.state == source.symbol)
                else {
                    eprintln!("Z41: no source_bindings for state");
                    return validation::RankArrivalCompatibility::Refuted;
                };
                let source_ready = !relevance.requires_rank(source.symbol)
                    || required
                        .iter()
                        .all(|coordinate| source_bindings.destination(coordinate).is_some());
                if !source_ready {
                    complete = false;
                    continue;
                }
                let Some(protected) = protected_input_paths(
                    program,
                    machine,
                    source,
                    Some(source_bindings),
                    premise_inputs,
                ) else {
                    eprintln!("Z41: protected_input_paths None (source {:?})", source.symbol);
                    return validation::RankArrivalCompatibility::Refuted;
                };
                let protected = protected.iter().map(String::as_str).collect::<Vec<_>>();
                for &target_position in &adjacency[source_position] {
                    if source_position == 0 && target_position == 0 {
                        continue;
                    }
                    let target = &states[target_position];
                    let Some(target_bindings) = selected
                        .iter()
                        .find(|selection| selection.state == target.symbol)
                    else {
                        return validation::RankArrivalCompatibility::Refuted;
                    };
                    let target_ready = !relevance.requires_rank(target.symbol)
                        || required
                            .iter()
                            .all(|coordinate| target_bindings.destination(coordinate).is_some());
                    if !target_ready {
                        complete = false;
                        continue;
                    }
                    let cyclic = components.iter().any(|component| {
                        component.contains(&source_position)
                            && component.contains(&target_position)
                            && graph::component_is_cyclic(&adjacency, component)
                    });
                    for edge in patterns::edges_to_state(program, source, target.symbol) {
                        let Some(prefix) = preserved_entry_prefix(
                            program,
                            machine,
                            source,
                            frames,
                            edge.statement_ordinal,
                            &protected,
                        ) else {
                            eprintln!("Z41: preserved_entry_prefix None src {:?} dst {:?} ord {}", source.symbol, target.symbol, edge.statement_ordinal);
                            return validation::RankArrivalCompatibility::Refuted;
                        };
                        // Even a rankless suffix keeps prefix/formation custody.
                        if !relevance.requires_rank(target.symbol) {
                            continue;
                        }
                        if !validation::ranking_range_arrival_preserves_roles(
                            program,
                            machine,
                            validation::RankingRangeState {
                                state: source,
                                entry_parameters: source_bindings,
                            },
                            validation::RankingRangeState {
                                state: target,
                                entry_parameters: target_bindings,
                            },
                            edge.statement_ordinal,
                            edge.arguments,
                        ) {
                            eprintln!("Z41: preserves_roles false src {:?} dst {:?} ord {}", source.symbol, target.symbol, edge.statement_ordinal);
                            return validation::RankArrivalCompatibility::Refuted;
                        }
                        let guards = edge
                            .guards
                            .iter()
                            .map(|guard| (guard.expression, guard.holds))
                            .collect::<Vec<_>>();
                        let edge_premises = if source_position == 0
                            && entry_is_initial
                            && matches!(premises, validation::RankingRangePremises::RankInvariant)
                        {
                            validation::RankingRangePremises::InitialEntry
                        } else {
                            premises
                        };
                        let Some(proof) = validation::prove_ranking_range_transition(
                            program,
                            machine,
                            range,
                            measure,
                            edge_premises,
                            validation::RankingRangeState {
                                state: source,
                                entry_parameters: source_bindings,
                            },
                            validation::RankingRangeState {
                                state: target,
                                entry_parameters: target_bindings,
                            },
                            &guards,
                            &prefix,
                            edge.arguments,
                        ) else {
                            eprintln!("Z41: prove_ranking_range_transition None src {:?} dst {:?} ord {}", source.symbol, target.symbol, edge.statement_ordinal);
                            return validation::RankArrivalCompatibility::Refuted;
                        };
                        if !proof.membership_and_pinning {
                            eprintln!("Z41: membership_and_pinning false src {:?} dst {:?}", source.symbol, target.symbol);
                            return validation::RankArrivalCompatibility::Refuted;
                        }
                        if cyclic && !proof.strictly_decreases {
                            eprintln!("Z41: not strictly_decreases src {:?} dst {:?}", source.symbol, target.symbol);
                            *failure = RankArrivalFailure::StrictDecrease {
                                source: source.symbol,
                                destination: target.symbol,
                            };
                            return validation::RankArrivalCompatibility::Refuted;
                        }
                        let Ok(statement_ordinal) = u32::try_from(edge.statement_ordinal) else {
                            return validation::RankArrivalCompatibility::Refuted;
                        };
                        edge_judgments.push(checked_trees::CheckedRankArrivalEdge {
                            source: source.symbol,
                            destination: target.symbol,
                            statement_ordinal,
                            target: edge.target,
                            non_increasing: proof.non_increasing,
                            strictly_decreases: proof.strictly_decreases,
                        });
                    }
                }
            }
            if complete {
                let mut edges = edge_judgments.clone();
                edges.extend_from_slice(root_edges);
                let candidate = super::RankArrivalAdmission {
                    premises,
                    states: selected.to_vec(),
                    origins: discovered.clone(),
                    edges,
                };
                if !accept(&candidate) {
                    return validation::RankArrivalCompatibility::Refuted;
                }
                accepted_edges = edge_judgments;
                validation::RankArrivalCompatibility::Complete
            } else {
                validation::RankArrivalCompatibility::Pending
            }
        },
    )?;
    Some(SelectedCorrespondence {
        states,
        origins: discovered,
        edges: accepted_edges,
    })
}
