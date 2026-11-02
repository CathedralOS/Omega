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
        application: Option<crate::IntegerApplicationFormation>,
        bindings: Vec<super::RuntimeRankApplicationBinding>,
    },
    Literal(u64),
}

impl RuntimeRankInput {
    pub fn retain(
        &self,
        plan: &mut checked_trees::CheckedRankArrivalPlan,
        program: &dyn crate::CheckingInput,
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
                upper: upper.retain(plan, program),
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
        program: &dyn crate::CheckingInput,
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
                application,
                bindings,
            } => {
                let formation = application
                    .as_ref()
                    .and_then(|application| {
                        let (machine, _) = crate::find_state(program, *state)?;
                        if application.owner() != *state || application.body() != *expression {
                            return None;
                        }
                        let rows = bindings
                            .iter()
                            .map(|binding| binding.retain(plan))
                            .collect::<Vec<_>>();
                        let bindings = plan.bindings.insert_many(rows);
                        let casts = plan
                            .application_casts
                            .insert_many(application.casts().iter().cloned());
                        let calls = plan
                            .application_calls
                            .insert_many(application.calls().iter().copied());
                        Some(plan.application_formations.insert(
                            checked_trees::CheckedRankApplicationFormation {
                                machine: machine.symbol,
                                machine_body: machine.states,
                                state: *state,
                                application: checked_trees::CheckedRankApplicationFacts {
                                    owner: application.owner(),
                                    definition_body: application.body(),
                                    casts,
                                    calls,
                                },
                                bindings,
                                // An expression has no structural input home.
                                // Its absence remains the record default when
                                // the retained home's API name changes.
                                ..Default::default()
                            },
                        ))
                    })
                    .unwrap_or_default();
                Operand::Expression {
                    state: *state,
                    expression: *expression,
                    primitive_type: *primitive_type,
                    formation,
                }
            }
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
                formation,
            } => {
                let (application, bindings) = if formation.is_valid() {
                    let row = plan.application_formations.get(*formation);
                    if row.state != *state
                        || row.application.owner != *state
                        || row.application.definition_body != *expression
                    {
                        return None;
                    }
                    let application = crate::IntegerApplicationFormation::from_retained(
                        row.application.owner,
                        row.application.definition_body,
                        plan.application_casts.span(row.application.casts)?.to_vec(),
                        plan.application_calls.span(row.application.calls)?.to_vec(),
                    )?;
                    let bindings = plan
                        .bindings
                        .span(row.bindings)?
                        .iter()
                        .map(|binding| {
                            super::RuntimeRankApplicationBinding::from_checked(plan, binding)
                        })
                        .collect::<Option<Vec<_>>>()?;
                    (Some(application), bindings)
                } else {
                    (None, Vec::new())
                };
                Self::Expression {
                    state: *state,
                    expression: *expression,
                    primitive_type: *primitive_type,
                    application,
                    bindings,
                }
            }
            Operand::Literal(value) => Self::Literal(*value),
        })
    }
}
