//! Publish only the complete source proof's coordinate choice. The retained
//! origins explain which completed occurrences supplied a candidate; they do
//! not claim edge coverage or equality. A selected suffix describes a reading
//! below that transported parent without manufacturing a source expression.
//! Fresh receiving completions have a separate roster without entry ancestry.
//! Preparation completes all fallible conversions before any arena insertion.

use super::RankArrivalAdmission;
use checked_trees::{
    CheckedMachineRankCorrespondence, CheckedRankArrivalPlan, CheckedRankCoordinate,
    CheckedRankCoordinateBinding, CheckedRankCoordinateHandle, CheckedRankCoordinateTransport,
    CheckedRankLocalCapture, CheckedRankPremiseRegime, CheckedRankReceivingCoordinate,
    CheckedStateRankCorrespondence,
};
use symbols::SymbolHandle;
use typed_trees::machine::Machine;

struct PreparedTransport {
    source: validation::RankSubjectCoordinate,
    selected_suffix: Vec<facts::PlaceSegment>,
    source_state: SymbolHandle,
    statement_ordinal: u32,
    target: typed_trees::statement::TransitionTargetHandle,
    actual: typed_trees::expression::ExpressionHandle,
    completion: typed_trees::expression::ExpressionHandle,
    derivation: checked_trees::CheckedRankValueDerivation,
    captures: Vec<CheckedRankLocalCapture>,
}

struct PreparedBinding {
    entry: validation::RankSubjectCoordinate,
    destination: validation::RankSubjectCoordinate,
    transports: Vec<PreparedTransport>,
    receiving: Vec<PreparedReceiving>,
}

struct PreparedReceiving {
    destination: validation::RankSubjectCoordinate,
    type_reference: typed_trees::types::TypeReferenceHandle,
    source_state: SymbolHandle,
    statement_ordinal: u32,
    target: typed_trees::statement::TransitionTargetHandle,
    actual: typed_trees::expression::ExpressionHandle,
    completion: typed_trees::expression::ExpressionHandle,
}

struct PreparedState {
    state: SymbolHandle,
    bindings: Vec<PreparedBinding>,
}

impl RankArrivalAdmission {
    pub(in crate::checks::termination) fn retain(
        self,
        program: &dyn validation::CheckingInput,
        plan: &mut CheckedRankArrivalPlan,
        machine: &Machine,
    ) -> Option<()> {
        if plan.for_body(machine.symbol, machine.states).is_some() {
            return None;
        }
        let entry_state = self.states.first()?.state;
        let premise_regime = match self.premises {
            validation::RankingRangePremises::RankInvariant => {
                CheckedRankPremiseRegime::RankInvariant
            }
            validation::RankingRangePremises::EntryInvariant => {
                CheckedRankPremiseRegime::EntryInvariant
            }
            validation::RankingRangePremises::InitialEntry => return None,
        };
        let mut prepared = Vec::new();
        for selected in self.states {
            let mut origins = self
                .origins
                .iter()
                .filter(|origin| origin.state == selected.state);
            let origin = origins.next()?;
            if origins.next().is_some() {
                return None;
            }
            let mut bindings = Vec::new();
            for selected in selected.coordinates {
                let mut transports = Vec::new();
                let mut authenticated_parent = false;
                for correspondence in &origin.coordinates {
                    if correspondence.entry.parameter != selected.entry.parameter {
                        continue;
                    }
                    let Some(suffix) = selected
                        .entry
                        .path
                        .strip_prefix(correspondence.entry.path.as_slice())
                    else {
                        continue;
                    };
                    let mut reading = correspondence.destination.clone();
                    reading.path.extend_from_slice(suffix);
                    if reading != selected.destination {
                        continue;
                    }
                    authenticated_parent = true;
                    for transport in &correspondence.transports {
                        let captures = transport
                            .captures
                            .iter()
                            .map(|capture| {
                                Some(CheckedRankLocalCapture {
                                    symbol: capture.symbol,
                                    statement_ordinal: u32::try_from(capture.statement_index)
                                        .ok()?,
                                    initializer: capture.initializer,
                                    occurrence: capture.occurrence,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        transports.push(PreparedTransport {
                            source: transport.source.clone(),
                            selected_suffix: suffix.to_vec(),
                            source_state: transport.source_state,
                            statement_ordinal: u32::try_from(transport.statement_index).ok()?,
                            target: transport.target,
                            actual: transport.actual,
                            completion: transport.expression,
                            derivation: transport.derivation,
                            captures,
                        });
                    }
                }
                let receiving = origin
                    .receiving_at_coordinate(program, &selected.destination)
                    .into_iter()
                    .map(|receiving| {
                        Some(PreparedReceiving {
                            destination: receiving.destination.clone(),
                            type_reference: receiving.type_reference,
                            source_state: receiving.source_state,
                            statement_ordinal: u32::try_from(receiving.statement_index).ok()?,
                            target: receiving.target,
                            actual: receiving.actual,
                            completion: receiving.expression,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                if !authenticated_parent && receiving.is_empty() {
                    return None;
                }
                bindings.push(PreparedBinding {
                    entry: selected.entry,
                    destination: selected.destination,
                    transports,
                    receiving,
                });
            }
            prepared.push(PreparedState {
                state: selected.state,
                bindings,
            });
        }
        // Every condition above belongs to publication preparation. From this
        // point onward insertion is infallible, so a refusal cannot leave a
        // partial body witness or partially authoritative arena rows.
        let mut states = Vec::new();
        for state in prepared {
            let mut bindings = Vec::new();
            for binding in state.bindings {
                let entry = insert_coordinate(plan, &binding.entry);
                let destination = insert_coordinate(plan, &binding.destination);
                let mut transports = Vec::new();
                for transport in binding.transports {
                    let source = insert_coordinate(plan, &transport.source);
                    let selected_suffix = plan.path_segments.insert_many(transport.selected_suffix);
                    let captures = plan.captures.insert_many(transport.captures);
                    transports.push(CheckedRankCoordinateTransport {
                        source_state: transport.source_state,
                        source,
                        selected_suffix,
                        statement_ordinal: transport.statement_ordinal,
                        target: transport.target,
                        actual: transport.actual,
                        completion: transport.completion,
                        derivation: transport.derivation,
                        captures,
                    });
                }
                let transports = plan.transports.insert_many(transports);
                let receiving = binding
                    .receiving
                    .into_iter()
                    .map(|receiving| CheckedRankReceivingCoordinate {
                        destination: insert_coordinate(plan, &receiving.destination),
                        type_reference: receiving.type_reference,
                        source_state: receiving.source_state,
                        statement_ordinal: receiving.statement_ordinal,
                        target: receiving.target,
                        actual: receiving.actual,
                        completion: receiving.completion,
                    })
                    .collect::<Vec<_>>();
                let receiving = plan.receiving.insert_many(receiving);
                bindings.push(CheckedRankCoordinateBinding {
                    entry,
                    destination,
                    transports,
                    receiving,
                });
            }
            let bindings = plan.bindings.insert_many(bindings);
            states.push(CheckedStateRankCorrespondence {
                state: state.state,
                bindings,
            });
        }
        let states = plan.states.insert_many(states);
        let edges = plan.arrival_edges.insert_many(self.edges);
        plan.machines.insert(CheckedMachineRankCorrespondence {
            machine: machine.symbol,
            body: machine.states,
            entry_state,
            premise_regime,
            states,
            edges,
        });
        Some(())
    }
}

fn insert_coordinate(
    plan: &mut CheckedRankArrivalPlan,
    coordinate: &validation::RankSubjectCoordinate,
) -> CheckedRankCoordinateHandle {
    let path = plan
        .path_segments
        .insert_many(coordinate.path.iter().cloned());
    plan.coordinates.insert(CheckedRankCoordinate {
        parameter: coordinate.parameter,
        path,
        entry_indexes: Default::default(),
    })
}
