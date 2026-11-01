//! Exact invocation coordinate observations and captured bound values.

pub(super) use super::super::rank_inputs::Coordinate;
use super::edge_observations::Edge;
use typed_trees::expression::ExpressionHandle;
use validation::ScopedArithmeticValue;

#[derive(Clone)]
pub(super) enum Bound {
    Authored {
        expression: ExpressionHandle,
        entry_value: ScopedArithmeticValue,
    },
    Coordinate(Coordinate),
    Closed(ScopedArithmeticValue),
}

#[derive(Clone)]
pub(super) struct Rank {
    pub(super) subject: super::subjects::Subject,
    pub(super) bound: Option<Bound>,
    pub(super) application: Option<super::applications::Application>,
    pub(super) entry_value: ScopedArithmeticValue,
    pub(super) formation: Option<validation::IntegerApplicationFormation>,
    pub(super) minimum: i128,
}

impl Bound {
    pub(super) fn entry(&self) -> &ScopedArithmeticValue {
        match self {
            Self::Authored { entry_value, .. } => entry_value,
            Self::Coordinate(coordinate) => &coordinate.entry_value,
            Self::Closed(value) => value,
        }
    }

    pub(super) fn arriving(
        &self,
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        edge: &Edge<'_>,
    ) -> Option<ScopedArithmeticValue> {
        match self {
            Self::Authored { expression, .. } => {
                let state = &program.machine_states(machine)[0];
                Some(ScopedArithmeticValue::Term(
                    edge.values.bind_call_parameters(
                        machine,
                        state,
                        *expression,
                        program.state_parameters(state),
                        edge.arguments,
                        edge.site,
                    )?,
                ))
            }
            Self::Coordinate(coordinate) => coordinate.current(
                &edge.values,
                *edge.arguments.get(coordinate.position)?,
                machine,
                edge.arguments,
            ),
            Self::Closed(value) => Some(value.clone()),
        }
    }
}

impl Bound {
    pub(super) fn observation(
        &self,
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
    ) -> Option<validation::RuntimeRankOperand> {
        use validation::RuntimeRankOperand as Operand;
        Some(match self {
            Self::Coordinate(coordinate) => Operand::Coordinate {
                parameter: coordinate.parameter,
                coordinate: validation::RankSubjectCoordinate {
                    parameter: coordinate.parameter,
                    path: coordinate.path.clone(),
                },
                entry_indexes: coordinate.entry_indexes.clone(),
                primitive_type: coordinate.primitive,
                observation: coordinate.observation,
            },
            Self::Closed(ScopedArithmeticValue::Integer(value)) => {
                Operand::Literal(value.to_u64()?)
            }
            Self::Closed(_) => return None,
            Self::Authored {
                expression,
                entry_value,
            } => {
                if let ScopedArithmeticValue::Integer(value) = entry_value {
                    Operand::Literal(value.to_u64()?)
                } else {
                    let reference = validation::mathematical_applications::operand_type_reference(
                        program,
                        *expression,
                    )?;
                    let primitive_type = program.primitive_type_reference(reference)?;
                    Operand::Expression {
                        state,
                        expression: *expression,
                        primitive_type,
                    }
                }
            }
        })
    }
}
