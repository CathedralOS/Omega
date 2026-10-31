//! Retain the admitted observation and its exact declared application inputs.
//! Application bindings are template substitutions, not arrival equalities;
//! the complete component's independently proved edges remain their authority.

pub(in crate::checks::termination) fn retain_observation(
    program: &dyn validation::CheckingInput,
    admitted: &mut checked_trees::CheckedRankArrivalPlan,
    input: &validation::ValidatedRuntimeNaturalState,
) -> Result<checked_trees::CheckedStateNaturalRank, &'static str> {
    let path = admitted
        .path_segments
        .insert_many(input.coordinate.path.clone());
    let entry_indexes = admitted
        .entry_indexes
        .insert_many(input.entry_indexes.clone());
    let coordinate = admitted
        .coordinates
        .insert(checked_trees::CheckedRankCoordinate {
            parameter: input.parameter,
            path,
            entry_indexes,
        });
    let measure = if input.measure.is_valid() {
        let formal = program
            .measures()
            .iter()
            .find(|definition| definition.symbol == input.measure)
            .and_then(|definition| definition.parameter.as_ref())
            .ok_or("an admitted rank application has no supplying declaration formal")?;
        let entry = admitted
            .coordinates
            .insert(checked_trees::CheckedRankCoordinate {
                parameter: formal.symbol,
                ..Default::default()
            });
        let bindings =
            admitted
                .bindings
                .insert_many([checked_trees::CheckedRankCoordinateBinding {
                    entry,
                    destination: coordinate,
                    ..Default::default()
                }]);
        checked_trees::CheckedNaturalRankMeasure::DeclaredMeasure {
            measure: input.measure,
            bindings,
            primitive_type: input.primitive_type,
        }
    } else {
        match input.observation {
            validation::RankPremiseObservation::Value => {
                checked_trees::CheckedNaturalRankMeasure::IntegerValue {
                    coordinate,
                    primitive_type: input.primitive_type,
                }
            }
            validation::RankPremiseObservation::Length => {
                checked_trees::CheckedNaturalRankMeasure::SequenceLength { coordinate }
            }
        }
    };
    Ok(checked_trees::CheckedStateNaturalRank {
        function: input.function,
        state: input.state,
        parameter: input.parameter,
        parameter_position: input.parameter_position,
        measure,
        observation: Default::default(),
    })
}
