//! Retain the admitted observation and its exact declared application inputs.
//! Application bindings are template substitutions, not arrival equalities;
//! the complete component's independently proved edges remain their authority.

pub(in crate::checks::termination) fn retain_observation(
    program: &dyn validation::CheckingInput,
    admitted: &mut checked_trees::CheckedRankArrivalPlan,
    input: &validation::ValidatedRuntimeNaturalState,
) -> Result<checked_trees::CheckedStateNaturalRank, &'static str> {
    if input.values.is_empty() {
        return Err("an admitted state rank has no ordered component observations");
    }
    let values = input
        .values
        .iter()
        .map(|value| retain_component(program, admitted, input.state, value))
        .collect::<Result<Vec<_>, _>>()?;
    let components = admitted.observation_components.insert_many(values);
    Ok(checked_trees::CheckedStateNaturalRank {
        state: input.state,
        components,
    })
}

fn retain_component(
    program: &dyn validation::CheckingInput,
    admitted: &mut checked_trees::CheckedRankArrivalPlan,
    state: symbols::SymbolHandle,
    input: &validation::ValidatedRuntimeIntegerRank,
) -> Result<checked_trees::CheckedIntegerRankObservation, &'static str> {
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
        let rows = input
            .application_bindings
            .iter()
            .map(|binding| binding.retain(admitted))
            .collect::<Vec<_>>();
        let bindings = admitted.bindings.insert_many(rows);
        let formation = if let Some(application) = &input.application {
            let (owner, _) = validation::find_state(program, state)
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
                    state,
                    application: checked_trees::CheckedRankApplicationFacts {
                        measure: application.measure(),
                        definition_body: application.body(),
                        casts,
                        calls: admitted
                            .application_calls
                            .insert_many(application.calls().iter().copied()),
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
        match &input.observation {
            validation::RuntimeRankInput::Value(validation::RankPremiseObservation::Value) => {
                checked_trees::CheckedNaturalRankMeasure::IntegerValue {
                    coordinate,
                    primitive_type: input.primitive_type,
                }
            }
            validation::RuntimeRankInput::Value(validation::RankPremiseObservation::Length) => {
                checked_trees::CheckedNaturalRankMeasure::SequenceLength { coordinate }
            }
            validation::RuntimeRankInput::Distance {
                lower_observation,
                lower_primitive,
                upper,
            } => {
                if input.function != checked_trees::CheckedNaturalRankFunction::Identity {
                    return Err("an exact distance cannot retain a complement normalization");
                }
                let lower =
                    admitted
                        .rank_operands
                        .insert(checked_trees::CheckedRankOperand::Coordinate {
                            coordinate,
                            primitive_type: *lower_primitive,
                            observation: *lower_observation,
                        });
                let upper = upper.retain(admitted);
                checked_trees::CheckedNaturalRankMeasure::Distance { lower, upper }
            }
        }
    };
    Ok(checked_trees::CheckedIntegerRankObservation {
        function: input.function,
        parameter: input.parameter,
        parameter_position: input.parameter_position,
        measure,
        observation: Default::default(),
    })
}
