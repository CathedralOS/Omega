//! Exact inputs of the scalar rank produced for one invocation.
//! These are transient checked-query records. Durable copies retain each
//! operand under the checked arrival plan's own coordinate/index arenas.

use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionHandle, types::PrimitiveType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeRankInput {
    Value(crate::RankPremiseObservation),
    Distance {
        lower_observation: crate::RankPremiseObservation,
        lower_primitive: PrimitiveType,
        upper: RuntimeRankOperand,
    },
}

impl From<crate::RankPremiseObservation> for RuntimeRankInput {
    fn from(value: crate::RankPremiseObservation) -> Self {
        Self::Value(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeRankOperand {
    Coordinate {
        parameter: SymbolHandle,
        coordinate: crate::RankSubjectCoordinate,
        entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
        primitive_type: PrimitiveType,
        observation: crate::RankPremiseObservation,
    },
    /// Already formed in this exact participant's own entry namespace.
    Expression {
        state: SymbolHandle,
        expression: ExpressionHandle,
        primitive_type: PrimitiveType,
    },
    Literal(u64),
}

impl RuntimeRankInput {
    pub fn retain(
        &self,
        plan: &mut checked_trees::CheckedRankArrivalPlan,
    ) -> checked_trees::CheckedRuntimeRankInput {
        use checked_trees::CheckedRuntimeRankInput as Input;
        match self {
            Self::Value(observation) => Input::Value(*observation),
            Self::Distance {
                lower_observation,
                lower_primitive,
                upper,
            } => Input::Distance {
                lower_observation: *lower_observation,
                lower_primitive: *lower_primitive,
                upper: upper.retain(plan),
            },
        }
    }

    pub fn from_checked(
        plan: &checked_trees::CheckedRankArrivalPlan,
        input: checked_trees::CheckedRuntimeRankInput,
    ) -> Option<Self> {
        use checked_trees::CheckedRuntimeRankInput as Input;
        Some(match input {
            Input::Value(observation) => Self::Value(observation),
            Input::Distance {
                lower_observation,
                lower_primitive,
                upper,
            } => Self::Distance {
                lower_observation,
                lower_primitive,
                upper: RuntimeRankOperand::from_checked(plan, upper)?,
            },
        })
    }
}

impl RuntimeRankOperand {
    pub fn matches_checked(
        &self,
        plan: &checked_trees::CheckedRankArrivalPlan,
        handle: arena::Handle<checked_trees::CheckedRankOperand>,
    ) -> bool {
        Self::from_checked(plan, handle).as_ref() == Some(self)
    }

    pub fn retain(
        &self,
        plan: &mut checked_trees::CheckedRankArrivalPlan,
    ) -> arena::Handle<checked_trees::CheckedRankOperand> {
        use checked_trees::CheckedRankOperand as Operand;
        let operand = match self {
            Self::Coordinate {
                parameter,
                coordinate,
                entry_indexes,
                primitive_type,
                observation,
            } => {
                let path = plan.path_segments.insert_many(coordinate.path.clone());
                let entry_indexes = plan.entry_indexes.insert_many(entry_indexes.clone());
                let coordinate = plan
                    .coordinates
                    .insert(checked_trees::CheckedRankCoordinate {
                        parameter: *parameter,
                        path,
                        entry_indexes,
                    });
                Operand::Coordinate {
                    coordinate,
                    primitive_type: *primitive_type,
                    observation: *observation,
                }
            }
            Self::Expression {
                state,
                expression,
                primitive_type,
            } => Operand::Expression {
                state: *state,
                expression: *expression,
                primitive_type: *primitive_type,
            },
            Self::Literal(value) => Operand::Literal(*value),
        };
        plan.rank_operands.insert(operand)
    }

    fn from_checked(
        plan: &checked_trees::CheckedRankArrivalPlan,
        handle: arena::Handle<checked_trees::CheckedRankOperand>,
    ) -> Option<Self> {
        use checked_trees::CheckedRankOperand as Operand;
        Some(match plan.rank_operands.get(handle) {
            Operand::Coordinate {
                coordinate,
                primitive_type,
                observation,
            } => {
                let coordinate = plan.coordinates.get(*coordinate);
                Self::Coordinate {
                    parameter: coordinate.parameter,
                    coordinate: crate::RankSubjectCoordinate {
                        parameter: coordinate.parameter,
                        path: plan.path_segments.span(coordinate.path)?.to_vec(),
                    },
                    entry_indexes: plan.entry_indexes.span(coordinate.entry_indexes)?.to_vec(),
                    primitive_type: *primitive_type,
                    observation: *observation,
                }
            }
            Operand::Expression {
                state,
                expression,
                primitive_type,
            } => Self::Expression {
                state: *state,
                expression: *expression,
                primitive_type: *primitive_type,
            },
            Operand::Literal(value) => Self::Literal(*value),
        })
    }
}
