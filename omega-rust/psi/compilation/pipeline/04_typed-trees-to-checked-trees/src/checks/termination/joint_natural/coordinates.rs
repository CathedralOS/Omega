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
    pub(super) coordinate: Coordinate,
    pub(super) bound: Option<Bound>,
    pub(super) application: Option<super::applications::Application>,
    pub(super) entry_value: ScopedArithmeticValue,
    pub(super) formation: Option<validation::IntegerApplicationFormation>,
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
