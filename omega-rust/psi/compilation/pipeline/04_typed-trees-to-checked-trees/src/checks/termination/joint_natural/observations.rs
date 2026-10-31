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
        let formation = if let Some(application) = &input.application {
            let (owner, _) = validation::find_state(program, input.state)
                .ok_or("a formed rank application lost its exact state owner")?;
            let declaration = program
                .measures()
                .iter()
                .find(|definition| definition.symbol == input.measure)
                .ok_or("a formed rank application lost its supplying declaration")?;
            if application.measure() != input.measure
                || program
                    .expression_table
                    .expression_handles(declaration.body)
                    != [application.body()]
            {
                return Err("a formed rank application changed its declaration or body");
            }
            let casts = admitted
                .application_casts
                .insert_many(application.casts().iter().cloned());
            admitted
                .application_formations
                .insert(checked_trees::CheckedRankApplicationFormation {
                    machine: owner.symbol,
                    machine_body: owner.states,
                    state: input.state,
                    application: checked_trees::CheckedRankApplicationFacts {
                        measure: application.measure(),
                        definition_body: application.body(),
                        casts,
                    },
                    bindings,
                })
        } else {
            Default::default()
        };
        checked_trees::CheckedNaturalRankMeasure::DeclaredMeasure {
            formation,
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
