//! Source positions join the active scalar and structural namespaces at the call leaf.
//! This is shared by Unit operations and scalar computations after operand evaluation.
use super::super::argument_evaluation::Evaluation;
use super::super::primitive_locals::PrimitiveLocal;
use super::{OperationEnvironment, OperationFrame};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::unit::{LoweringError, ValueDeclaration, terminal_scalar_type, unsupported};

struct FrontierSource<'a, 'context> {
    checked: &'a checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    scalar_parameter_count: usize,
    primitive_locals: &'a [PrimitiveLocal],
    evaluation: &'a Evaluation,
    values: &'a [ValueDeclaration],
    calls: &'a CallEmissionContext<'context>,
    operations: &'a OperationBuffer,
}
impl OperationFrame<'_, '_> {
    pub(super) fn suspension_source_frontier(
        &self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
    ) -> Result<lowered_psi::LoweredSourceFrontierBindings, LoweringError> {
        retain(
            FrontierSource {
                checked: self.checked,
                machine: self.machine,
                state: self.state,
                scalar_parameter_count: self.scalar_parameter_count,
                primitive_locals: self.primitive_locals,
                evaluation: self.evaluation,
                values: self.values,
                calls: self.calls,
                operations: self.operations,
            },
            coordinate,
        )
    }
}
impl OperationEnvironment<'_> {
    pub(in crate::unit::attached_unit) fn suspension_source_frontier(
        &self,
        coordinate: SourceCallCoordinate,
        evaluation: &Evaluation,
        values: &[ValueDeclaration],
        calls: &CallEmissionContext<'_>,
        operations: &OperationBuffer,
    ) -> Result<lowered_psi::LoweredSourceFrontierBindings, LoweringError> {
        if coordinate.state != self.state {
            return unsupported("suspension scalar call changes its authored source state");
        }
        let coordinate = checked_trees::CheckedUnitCallCoordinate {
            statement_index: u32::try_from(coordinate.statement_index)
                .map_err(|_| LoweringError::Unsupported("suspension statement index overflows"))?,
            call_ordinal: u32::try_from(coordinate.call_ordinal)
                .map_err(|_| LoweringError::Unsupported("suspension call ordinal overflows"))?,
        };
        retain(
            FrontierSource {
                checked: self.checked,
                machine: self.machine,
                state: self.state,
                scalar_parameter_count: self.scalar_parameter_count,
                primitive_locals: self.primitive_locals,
                evaluation,
                values,
                calls,
                operations,
            },
            coordinate,
        )
    }
}
fn retain(
    source: FrontierSource<'_, '_>,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> Result<lowered_psi::LoweredSourceFrontierBindings, LoweringError> {
    let Some(crossing) = source
        .checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .find(|crossing| {
            crossing.machine == source.machine
                && crossing.state == source.state
                && crossing.statement_index == coordinate.statement_index as usize
                && crossing.call_ordinal == coordinate.call_ordinal as usize
        })
    else {
        return Ok(Default::default());
    };
    let (_, state) = source
        .checked
        .state_flow_site(source.machine, source.state)
        .ok_or(LoweringError::Unsupported(
            "suspension call lost its checked source state",
        ))?;
    let parameters = source.checked.state_parameters(state);
    let locals = source.checked.facts.state_locals.locals(source.state);
    let mut bindings = lowered_psi::LoweredSourceFrontierBindings {
        claims: source.calls.claims.bindings().to_vec(),
        ..Default::default()
    };
    for capture in &source.operations.loan_captures {
        if source
            .checked
            .facts
            .carry
            .suspension_loans
            .span_or_empty(crossing.borrows.loans)
            .iter()
            .any(|entry| entry.retained_for_crossing && entry.stored_loan() == capture.loan)
        {
            bindings.loan_captures.push(capture.clone());
        }
    }
    for live in &crossing.live_values {
        let (symbol, source_position) = match live.origin {
            checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, position } => {
                if !parameters
                    .get(position)
                    .is_some_and(|parameter| parameter.symbol == symbol)
                {
                    return unsupported("suspension parameter changes its checked source position");
                }
                (symbol, position)
            }
            checked_trees::SuspensionCrossingValueOrigin::Local {
                symbol,
                statement_index,
                environment_position,
            } => {
                if !locals.iter().enumerate().any(|(position, local)| {
                    local.symbol == symbol
                        && local.statement_index as usize == statement_index
                        && local.statement_index < coordinate.statement_index
                        && parameters.len() + position == environment_position
                }) {
                    return unsupported("suspension local changes its checked source position");
                }
                (symbol, environment_position)
            }
            checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => continue,
        };
        // Storage identity comes before carrier classification. An ordinary
        // scalar binding may now occupy an addressable home; retaining its
        // initializer SSA value would lose intervening writes and its address.
        let local = crate::expression_preparation::bindings::structural_local_by_root(
            &source.evaluation.structural_locals,
            facts::PlaceRoot::Symbol(symbol),
        );
        let home = if let Some(local) = local {
            Some((local.argument.clone(), local.structural_type))
        } else if let Some((_, place, scalar_type)) = source
            .evaluation
            .primitive_storage
            .iter()
            .find(|(candidate, _, _)| *candidate == symbol)
        {
            let declaration = source
                .primitive_locals
                .iter()
                .chain(&source.evaluation.parameter_storage)
                .find(|local| local.declaration.id == *place)
                .ok_or(LoweringError::Unsupported(
                    "suspension scalar home has no established declaration",
                ))?;
            let (_, structural_type) = declaration.declaration.kind.as_operation_result().ok_or(
                LoweringError::Unsupported("suspension scalar home has no establishing operation"),
            )?;
            let primitive = source
                .checked
                .primitive_type_reference(live.type_reference)
                .ok_or(LoweringError::Unsupported(
                    "suspension scalar home has no checked carrier",
                ))?;
            if terminal_scalar_type(primitive)? != *scalar_type {
                return unsupported("suspension scalar home changes its checked carrier");
            }
            Some((
                terminal_psi::StructuralArgument {
                    place: *place,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                },
                structural_type,
            ))
        } else if source
            .checked
            .primitive_type_reference(live.type_reference)
            .is_none()
        {
            source
                .evaluation
                .structural_parameters
                .iter()
                .find(|(position, _)| *position as usize == source_position)
                .map(|(_, parameter)| {
                    (
                        terminal_psi::StructuralArgument {
                            place: parameter.place,
                            path: Vec::new(),
                            access: parameter.access,
                        },
                        parameter.structural_type,
                    )
                })
        } else {
            None
        };
        if let Some((argument, structural_type)) = home {
            bindings
                .structural
                .push(lowered_psi::LoweredSourceStructuralBinding {
                    source_position,
                    argument,
                    structural_type,
                });
            continue;
        }
        let authored_position = match live.origin {
            checked_trees::SuspensionCrossingValueOrigin::Parameter { position, .. } => {
                let parameter = &parameters[position];
                if parameter.is_mutable
                    || parameter.relevance.is_erased()
                    || source
                        .checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
                {
                    continue;
                }
                parameters[..position]
                    .iter()
                    .filter(|parameter| {
                        !parameter.relevance.is_erased()
                            && source
                                .checked
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })
                    .count()
            }
            checked_trees::SuspensionCrossingValueOrigin::Local {
                environment_position,
                ..
            } => {
                let local = &locals[environment_position - parameters.len()];
                if !local.is_scalar_value {
                    continue;
                }
                source.scalar_parameter_count
                    + source
                        .checked
                        .facts
                        .state_locals
                        .scalar_locals_before(source.state, local.statement_index)
                        as usize
            }
            checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => continue,
        };
        let Some((value_position, scalar_type)) = source
            .evaluation
            .scalar_bindings
            .unaddressed_value(authored_position)
        else {
            continue;
        };
        let value =
            source
                .values
                .get(value_position)
                .copied()
                .ok_or(LoweringError::Unsupported(
                    "suspension scalar binding lost its emitted value",
                ))?;
        if value.scalar_type != scalar_type {
            return unsupported("suspension scalar binding changes its carrier");
        }
        bindings
            .scalars
            .push(lowered_psi::LoweredSourceScalarBinding {
                source_position,
                value,
            });
    }
    Ok(bindings)
}
