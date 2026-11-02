//! Read retained rank inputs through the ordinary scalar graph. Each operand
//! keeps its own carrier; exact distance produces the common unsigned word.

use super::{Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind};
use checked_trees::{
    CheckedNaturalRankMeasure, CheckedRankArrivalPlan, CheckedRankOperand, CheckedScalarExpression,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::{
    expression::{BinaryOperator, ExpressionHandle},
    signature::StateParameter,
    types::PrimitiveType,
};

pub(super) fn declaration_parameters(
    program: &dyn validation::CheckingInput,
    admitted: &CheckedRankArrivalPlan,
    measure: &CheckedNaturalRankMeasure,
    inputs: &[StateParameter],
) -> Option<Vec<StateParameter>> {
    let mut parameters = inputs.to_vec();
    if let CheckedNaturalRankMeasure::Distance { lower, upper } = measure {
        for input in [*lower, *upper] {
            if let CheckedRankOperand::Expression { state, .. } = admitted.rank_operands.get(input)
            {
                let (_, state) = validation::find_state(program, *state)?;
                for parameter in program.state_parameters(state).iter() {
                    if !parameters
                        .iter()
                        .any(|known| known.symbol == parameter.symbol)
                    {
                        parameters.push(parameter.clone());
                    }
                }
            }
        }
    }
    Some(parameters)
}

pub(super) fn difference(
    builder: &mut Builder<'_, '_>,
    admitted: &CheckedRankArrivalPlan,
    lower: arena::Handle<CheckedRankOperand>,
    upper: arena::Handle<CheckedRankOperand>,
) -> Option<CheckedScalarComputationHandle> {
    let lower = read(builder, admitted, lower)?;
    let upper = read(builder, admitted, upper)?;
    let primitive_type = PrimitiveType::U64;
    let parameter = |position| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let (expression, _) = crate::values::scalar::scalar_lowering::construct_integer_binary(
        BinaryOperator::Subtract,
        parameter(0),
        ArithmeticDomain::Exact,
        parameter(1),
        ArithmeticDomain::Exact,
        true,
        false,
    )?;
    let operands = builder.plans.operands.insert_many([upper, lower]);
    Some(builder.insert(
        primitive_type,
        CheckedScalarComputationKind::Apply {
            source_expression: ExpressionHandle::invalid(),
            expression,
            operands,
        },
    ))
}

fn read(
    builder: &mut Builder<'_, '_>,
    admitted: &CheckedRankArrivalPlan,
    input: arena::Handle<CheckedRankOperand>,
) -> Option<CheckedScalarComputationHandle> {
    let input = admitted.rank_operands.get(input);
    let (value, primitive_type) = match input {
        CheckedRankOperand::Coordinate {
            coordinate,
            primitive_type,
            observation,
        } => {
            let coordinate = admitted.coordinates.get(*coordinate);
            let place = validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(coordinate.parameter),
                segments: admitted.path_segments.span(coordinate.path)?.to_vec(),
            };
            let value = match observation {
                checked_trees::CheckedRankObservation::Value => builder.scalar_coordinate(
                    ExpressionHandle::invalid(),
                    &place,
                    *primitive_type,
                    admitted.entry_indexes.span(coordinate.entry_indexes)?,
                )?,
                checked_trees::CheckedRankObservation::Length => {
                    let (_, state) = validation::find_state(builder.program, builder.state)?;
                    let (_, _, value) = validation::ranking_slice_length_observation(
                        builder.program,
                        state,
                        &validation::RankSubjectCoordinate {
                            parameter: coordinate.parameter,
                            path: place.segments.clone(),
                        },
                    )?;
                    builder.insert(
                        PrimitiveType::U64,
                        CheckedScalarComputationKind::Value(value),
                    )
                }
            };
            (value, *primitive_type)
        }
        CheckedRankOperand::Expression {
            expression,
            primitive_type,
            ..
        } => (
            builder.expression(*expression, *primitive_type)?,
            *primitive_type,
        ),
        CheckedRankOperand::Literal(value) => {
            let value = builder.insert(
                PrimitiveType::U64,
                CheckedScalarComputationKind::Value(facts::u64_integer_literal(
                    *value,
                    PrimitiveType::U64,
                )?),
            );
            (value, PrimitiveType::U64)
        }
    };
    if primitive_type == PrimitiveType::U64 {
        return Some(value);
    }
    let operand = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let expression = if matches!(
        primitive_type,
        typed_trees::unsigned_integer_primitive_pattern!()
    ) {
        CheckedScalarExpression::IntegerWiden {
            primitive_type: PrimitiveType::U64,
            operand: Box::new(operand),
        }
    } else {
        // The admitted natural difference has an unsigned lower endpoint and
        // independently proves lower<=upper. This upper is therefore in the
        // intersection of its own fixed carrier with the nonnegative words.
        let (_, maximum) = primitive_type.integer_bounds()?;
        if maximum > i128::from(u64::MAX) {
            return None;
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type: PrimitiveType::U64,
            operand: Box::new(operand),
            range: checked_trees::CheckedIntegerRange {
                minimum: numerics::bignum::BigInt::zero(),
                maximum: numerics::bignum::BigInt::from_i128(maximum),
            },
            formation_decided: true,
        }
    };
    let operands = builder.plans.operands.insert_many([value]);
    Some(builder.insert(
        PrimitiveType::U64,
        CheckedScalarComputationKind::Apply {
            source_expression: ExpressionHandle::invalid(),
            expression,
            operands,
        },
    ))
}
