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
        formation: validation::IntegerApplicationFormation,
        bindings: Vec<validation::RuntimeRankApplicationBinding>,
    },
    /// A peer's authored endpoint read from this participant. `formals`
    /// pairs each formal position the owner's expression reads with this
    /// participant's parameter position that the owner receives it from.
    Transported {
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        formals: Vec<(usize, usize)>,
        entry_value: ScopedArithmeticValue,
        formation: validation::IntegerApplicationFormation,
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
            Self::Authored { entry_value, .. } | Self::Transported { entry_value, .. } => {
                entry_value
            }
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
                let expression = edge.values.bind_integer_expression_parameters(
                    machine,
                    state,
                    *expression,
                    edge.arguments,
                )?;
                edge.values
                    .integer_expression_formation(edge.values.machine(), &expression)
                    .map(|(_, value)| value)
            }
            Self::Transported {
                owner,
                expression,
                formals,
                ..
            } => {
                let (owner_machine, owner_state) = validation::find_state(program, *owner)?;
                let (parameters, arguments) =
                    transported_roster(program, owner_state, formals, edge.arguments)?;
                let expression = edge.values.bind_integer_expression_formals(
                    owner_machine,
                    owner_state,
                    *expression,
                    &parameters,
                    &arguments,
                )?;
                edge.values
                    .integer_expression_formation(edge.values.machine(), &expression)
                    .map(|(_, value)| value)
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
                formation,
                bindings,
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
                        evaluator: state,
                        expression: *expression,
                        primitive_type,
                        application: Some(formation.clone()),
                        bindings: bindings.clone(),
                    }
                }
            }
            Self::Transported {
                owner,
                expression,
                formals,
                entry_value,
                formation,
            } => {
                if let ScopedArithmeticValue::Integer(value) = entry_value {
                    Operand::Literal(value.to_u64()?)
                } else {
                    let (_, owner_state) = validation::find_state(program, *owner)?;
                    let (_, evaluator) = validation::find_state(program, state)?;
                    let reference = validation::mathematical_applications::operand_type_reference(
                        program,
                        *expression,
                    )?;
                    let primitive_type = program.primitive_type_reference(reference)?;
                    let formal_parameters = program.state_parameters(owner_state);
                    let actual_parameters = program.state_parameters(evaluator);
                    let bindings = formals
                        .iter()
                        .map(|(formal, actual)| {
                            let coordinate = |parameter: symbols::SymbolHandle| {
                                validation::RankSubjectCoordinate {
                                    parameter,
                                    path: Vec::new(),
                                }
                            };
                            Some(validation::RuntimeRankApplicationBinding {
                                calls: Vec::new(),
                                formal: coordinate(formal_parameters.get(*formal)?.symbol),
                                actual: coordinate(actual_parameters.get(*actual)?.symbol),
                                entry_indexes: Vec::new(),
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Operand::Expression {
                        state: *owner,
                        evaluator: state,
                        expression: *expression,
                        primitive_type,
                        application: Some(formation.clone()),
                        bindings,
                    }
                }
            }
        })
    }
}

/// The owner's formals a transported endpoint reads, each paired with the
/// actual this participant's caller supplies for the matching parameter.
pub(super) fn transported_roster(
    program: &dyn validation::CheckingInput,
    owner: &typed_trees::state::State,
    formals: &[(usize, usize)],
    arguments: &[ExpressionHandle],
) -> Option<(
    Vec<typed_trees::signature::StateParameter>,
    Vec<ExpressionHandle>,
)> {
    let parameters = program.state_parameters(owner);
    formals
        .iter()
        .map(|(formal, actual)| Some((parameters.get(*formal)?.clone(), *arguments.get(*actual)?)))
        .collect::<Option<Vec<_>>>()
        .map(|pairs| pairs.into_iter().unzip())
}
