//! Typed rank input coordinates and their captured selector/load receipts.
//! Entry discovery and arriving reads share the ordinary scalar-transfer owner.

use crate::checks::current_scalar_values::CurrentScalarValues;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::PrimitiveType;
use validation::ScopedArithmeticValue;

#[derive(Clone)]
pub(in crate::checks::termination) struct Coordinate {
    pub(in crate::checks::termination) parameter: SymbolHandle,
    pub(in crate::checks::termination) path: Vec<facts::PlaceSegment>,
    pub(in crate::checks::termination) entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
    pub(in crate::checks::termination) entry_value: ScopedArithmeticValue,
    pub(in crate::checks::termination) position: usize,
    pub(in crate::checks::termination) primitive: PrimitiveType,
    pub(in crate::checks::termination) observation: validation::RankPremiseObservation,
}

impl Coordinate {
    pub(in crate::checks::termination) fn current(
        &self,
        values: &CurrentScalarValues<'_>,
        actual: ExpressionHandle,
        callee: &typed_trees::machine::Machine,
        arguments: &[ExpressionHandle],
    ) -> Option<ScopedArithmeticValue> {
        match self.observation {
            validation::RankPremiseObservation::Value => {
                let state = values.program().machine_states(callee).first()?;
                values.projected_scalar_value_in_call(
                    actual,
                    &self.path,
                    callee,
                    state,
                    values.program().state_parameters(state),
                    arguments,
                )
            }
            validation::RankPremiseObservation::Length => {
                values.projected_collection_length(actual, &self.path)
            }
        }
    }
}

/// Capture eligible coordinates once before the first admitted executable
/// statement. An invalid target applies no branch guard or operand effects.
/// Each candidate owns only its selected value, not a second field roster.
pub(in crate::checks::termination) fn coordinates(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    values: &CurrentScalarValues<'_>,
) -> Vec<Coordinate> {
    let mut coordinates = Vec::new();
    for observation in [
        validation::RankPremiseObservation::Value,
        validation::RankPremiseObservation::Length,
    ] {
        for (position, parameter) in program.state_parameters(state).iter().enumerate() {
            if parameter.relevance.is_erased() || parameter.is_const {
                continue;
            }
            if let Some(primitive) =
                observation_carrier(program, state, parameter.symbol, observation)
            {
                let entry_value = match observation {
                    validation::RankPremiseObservation::Value => {
                        values.initial_symbol(parameter.symbol)
                    }
                    validation::RankPremiseObservation::Length => {
                        values.initial_collection_length(parameter.symbol)
                    }
                };
                if let Some(entry_value) = entry_value {
                    coordinates.push(Coordinate {
                        parameter: parameter.symbol,
                        path: Vec::new(),
                        entry_indexes: Vec::new(),
                        entry_value,
                        position,
                        primitive,
                        observation,
                    });
                }
            }
            // Entry seeds retain ordinary fields under their admitted root access.
            // The runtime projection owner independently authenticates each selected
            // scalar path, including erasure and exact declaration identities.
            if observation == validation::RankPremiseObservation::Value {
                for (path, primitive, value, entry_indexes) in
                    values.entry_scalar_field_observations(parameter.symbol)
                {
                    let Some(field) = validation::structural_scalar_coordinate(
                        program,
                        program.state_parameters(state),
                        parameter.symbol,
                        &path,
                    ) else {
                        continue;
                    };
                    if field.primitive_type != primitive {
                        continue;
                    }
                    coordinates.push(Coordinate {
                        parameter: parameter.symbol,
                        path,
                        entry_indexes,
                        entry_value: value.clone(),
                        position,
                        primitive,
                        observation,
                    });
                }
            } else {
                for (path, value) in values.length_field_observations(parameter.symbol) {
                    let path = path
                        .iter()
                        .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                        .collect::<Vec<_>>();
                    // Extents also include fixed arrays. The ordinary coordinate
                    // owner authenticates that this selected leaf is a descriptor
                    // observation before it can enter a Slice::Length component.
                    if validation::ranking_slice_length_observation(
                        program,
                        state,
                        &validation::RankSubjectCoordinate {
                            parameter: parameter.symbol,
                            path: path.clone(),
                        },
                    )
                    .is_none()
                    {
                        continue;
                    }
                    coordinates.push(Coordinate {
                        parameter: parameter.symbol,
                        path,
                        entry_indexes: Vec::new(),
                        entry_value: value.clone(),
                        position,
                        primitive: PrimitiveType::U64,
                        observation,
                    });
                }
            }
        }
    }
    coordinates
}

fn observation_carrier(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    parameter: SymbolHandle,
    observation: validation::RankPremiseObservation,
) -> Option<PrimitiveType> {
    match observation {
        validation::RankPremiseObservation::Value => {
            let parameter = program.state_parameter_by_symbol(state, parameter)?;
            let primitive = program.primitive_type_reference(parameter.type_reference)?;
            primitive.is_fixed_integer().then_some(primitive)
        }
        validation::RankPremiseObservation::Length => {
            validation::ranking_slice_length_observation(
                program,
                state,
                &validation::RankSubjectCoordinate {
                    parameter,
                    path: Vec::new(),
                },
            )?;
            Some(PrimitiveType::U64)
        }
    }
}
