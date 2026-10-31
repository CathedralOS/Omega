//! Read the source-admitted ordered rank roster, preserve every component's
//! coordinate association, then bind its scalar roots into this executable
//! control plan. Admission prototypes and executable spans have distinct arena
//! owners; fusion changes placement, never tuple order or source meaning.

use super::{FusedStates, fused_states};

/// Read one retained source observation roster for the authored owners in this
/// composed graph. Fusion changes control placement, never the chosen measure
/// coordinate or the source judgment. Weak call-component edges come from its
/// same retained complete decision.
pub(crate) fn natural_rank_plans(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> Option<(
    Vec<checked_trees::CheckedStateNaturalRank>,
    Vec<checked_trees::CheckedRankEdgeCoordinate>,
)> {
    let fused = fused_states(program, machine);
    natural_rank_plans_for_fused(program, &fused, admitted)
}

pub(super) fn natural_rank_plans_for_fused(
    program: &dyn validation::CheckingInput,
    fused: &FusedStates<'_>,
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> Option<(
    Vec<checked_trees::CheckedStateNaturalRank>,
    Vec<checked_trees::CheckedRankEdgeCoordinate>,
)> {
    let has_witness = fused
        .iter()
        .any(|(owner, _)| owner.termination_plan.implementation_witness.is_some());
    if !has_witness && admitted.runtime_components.is_empty() {
        return Some((Vec::new(), Vec::new()));
    }
    let members = fused
        .iter()
        .map(|(_, state)| state.symbol)
        .collect::<Vec<_>>();
    let mut ranks = Vec::new();
    for (owner, state) in fused {
        let judgment = admitted.judgment_for_body(owner.symbol, owner.states)?;
        // A private component member can carry a source-proved coordinate
        // without authoring a second ranking clause. Read the retained
        // observation roster rather than reconstructing declaration evidence.
        if owner.termination_plan.implementation_witness.is_none()
            && judgment.observations.is_empty()
        {
            continue;
        }
        if !judgment.local_terminates {
            return None;
        }
        let observations = admitted.observations.span(judgment.observations)?;
        let relevance = validation::rank_relevance(program, &[owner.symbol])?;
        if relevance.requires_rank(state.symbol)
            && !observations.iter().any(|rank| rank.state == state.symbol)
        {
            return None;
        }
        ranks.extend(
            observations
                .iter()
                .filter(|rank| rank.state == state.symbol)
                .cloned(),
        );
    }
    ranks.sort_by_key(|rank| (rank.state.arena_index(), rank.state.generation()));
    for same_state in ranks.windows(2) {
        if same_state[0].state == same_state[1].state && same_state[0] != same_state[1] {
            return None;
        }
    }
    ranks.dedup();
    if admitted.runtime_components.is_empty() {
        return Some((ranks, Vec::new()));
    }
    let mut weak = Vec::new();
    for component in validation::retained_runtime_natural_components(admitted) {
        if !component
            .states
            .iter()
            .any(|rank| members.contains(&rank.state))
            && !component
                .edges
                .iter()
                .any(|edge| members.contains(&edge.source) || members.contains(&edge.target))
        {
            continue;
        }
        if !component
            .states
            .iter()
            .all(|rank| members.contains(&rank.state))
            || !component
                .edges
                .iter()
                .all(|edge| members.contains(&edge.source) && members.contains(&edge.target))
        {
            return None;
        }
        // Every projected component state keeps the exact source observation
        // which its accepted coordinate witness produced.
        if component.states.iter().any(|state| {
            !ranks
                .iter()
                .filter(|rank| rank.state == state.state)
                .any(|root| {
                    let Some(values) = admitted.observation_components.span(root.components) else {
                        return false;
                    };
                    !values.is_empty()
                        && values.len() == state.values.len()
                        && values.iter().zip(&state.values).all(|(rank, state)| {
                            rank.parameter == state.parameter
                                && rank.parameter_position == state.parameter_position
                                && rank.function == state.function
                                && match (&state.observation, rank.measure) {
                                    (
                                        validation::RuntimeRankInput::Value(
                                            validation::RankPremiseObservation::Value,
                                        ),
                                        checked_trees::CheckedNaturalRankMeasure::DeclaredMeasure {
                                            measure,
                                            bindings,
                                            primitive_type,
                                            ..
                                        },
                                    ) => {
                                        let Some([binding]) = admitted.bindings.span(bindings)
                                        else {
                                            return false;
                                        };
                                        let input = admitted.coordinates.get(binding.destination);
                                        let formal = admitted.coordinates.get(binding.entry);
                                        measure == state.measure
                                            && measure.is_valid()
                                            && primitive_type == state.primitive_type
                                            && program
                                                .measures()
                                                .iter()
                                                .find(|definition| definition.symbol == measure)
                                                .and_then(|definition| {
                                                    definition.parameter.as_ref()
                                                })
                                                .is_some_and(|parameter| {
                                                    parameter.symbol == formal.parameter
                                                })
                                            && formal.path.is_empty()
                                            && input.parameter == state.coordinate.parameter
                                            && admitted.path_segments.span(input.path)
                                                == Some(state.coordinate.path.as_slice())
                                            && admitted.entry_indexes.span(input.entry_indexes)
                                                == Some(state.entry_indexes.as_slice())
                                    }
                                    (
                                        validation::RuntimeRankInput::Value(
                                            validation::RankPremiseObservation::Value,
                                        ),
                                        checked_trees::CheckedNaturalRankMeasure::IntegerValue {
                                            coordinate,
                                            primitive_type,
                                        },
                                    ) => {
                                        let coordinate = admitted.coordinates.get(coordinate);
                                        !state.measure.is_valid()
                                            && primitive_type == state.primitive_type
                                            && coordinate.parameter == state.coordinate.parameter
                                            && admitted.path_segments.span(coordinate.path)
                                                == Some(state.coordinate.path.as_slice())
                                    }
                                    (
                                        validation::RuntimeRankInput::Value(
                                            validation::RankPremiseObservation::Length,
                                        ),
                                        checked_trees::CheckedNaturalRankMeasure::SequenceLength {
                                            coordinate,
                                        },
                                    ) => {
                                        let coordinate = admitted.coordinates.get(coordinate);
                                        coordinate.parameter == state.coordinate.parameter
                                            && admitted.path_segments.span(coordinate.path)
                                                == Some(state.coordinate.path.as_slice())
                                    }
                                    (
                                        validation::RuntimeRankInput::Distance {
                                            lower_observation,
                                            lower_primitive,
                                            upper: expected_upper,
                                        },
                                        checked_trees::CheckedNaturalRankMeasure::Distance {
                                            lower,
                                            upper,
                                        },
                                    ) => {
                                        let expected_lower =
                                            validation::RuntimeRankOperand::Coordinate {
                                                parameter: state.parameter,
                                                coordinate: state.coordinate.clone(),
                                                entry_indexes: state.entry_indexes.clone(),
                                                primitive_type: *lower_primitive,
                                                observation: *lower_observation,
                                            };
                                        !state.measure.is_valid()
                                            && expected_lower.matches_checked(admitted, lower)
                                            && expected_upper.matches_checked(admitted, upper)
                                    }
                                    _ => false,
                                }
                        })
                })
        }) {
            return None;
        }
        weak.extend(
            component
                .edges
                .iter()
                .filter(|edge| !edge.strict)
                .map(|edge| checked_trees::CheckedRankEdgeCoordinate {
                    source_state: edge.source,
                    target_state: edge.target,
                    statement_ordinal: edge.statement_ordinal,
                }),
        );
    }
    weak.sort_by_key(|edge| {
        (
            edge.source_state.arena_index(),
            edge.source_state.generation(),
            edge.statement_ordinal,
            edge.target_state.arena_index(),
            edge.target_state.generation(),
        )
    });
    weak.dedup();
    Some((ranks, weak))
}

/// Rebind every component into the composed plan's own arena. A missing root
/// rejects the whole tuple; it cannot shorten the relation to a passing prefix.
pub(super) fn bind_observations(
    program: &dyn validation::CheckingInput,
    admitted: &checked_trees::CheckedRankArrivalPlan,
    computations: &checked_trees::CheckedScalarComputationPlans,
    ranks: &mut [checked_trees::CheckedStateNaturalRank],
) -> Option<arena::Arena<checked_trees::CheckedIntegerRankObservation>> {
    let mut components = arena::Arena::new();
    for rank in ranks {
        let (owner, _) = validation::find_state(program, rank.state)?;
        let prototype = admitted.observation_components.span(rank.components)?;
        if prototype.is_empty() {
            return None;
        }
        let values = prototype
            .iter()
            .enumerate()
            .map(|(component, value)| {
                let root = computations.root_at(
                    rank.state,
                    0,
                    checked_trees::CheckedScalarExpressionRole::RankObservation {
                        component: u32::try_from(component).ok()?,
                    },
                )?;
                if root.machine != owner.symbol {
                    return None;
                }
                let mut value = value.clone();
                value.observation = root.root;
                Some(value)
            })
            .collect::<Option<Vec<_>>>()?;
        rank.components = components.insert_many(values);
    }
    Some(components)
}
