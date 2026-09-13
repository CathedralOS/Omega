//! Non-scalar graph effects consume completed operands without adding value slots.

use super::*;

pub(crate) fn emit(
    effects: &[LoweredScalarEffect],
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<(), LoweringError> {
    for effect in effects {
        match effect {
            LoweredScalarEffect::StoreScalarField {
                destination,
                path,
                field,
                value_position,
                scalar_type,
            } => {
                let value = values
                    .get(*value_position)
                    .filter(|value| value.scalar_type == *scalar_type)
                    .ok_or(LoweringError::Unsupported(
                        "record store lost its completed scalar operand",
                    ))?;
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    id,
                    result: OperationResult::Unit,
                    kind: OperationKind::StructuralScalarFieldStore {
                        destination: *destination,
                        path: path.clone(),
                        field: *field,
                        value: value.id,
                    },
                });
            }
            LoweredScalarEffect::EstablishRecord(record) => {
                crate::scalar_graph_lowering::structural_values::emit(
                    record, values, operations, calls,
                )?;
            }
            LoweredScalarEffect::EstablishScalarCase(case) => {
                crate::scalar_computations::cases::emit(case, values, next_value, operations)?
            }
            LoweredScalarEffect::EstablishScalarArray(array) => {
                crate::scalar_computations::arrays::emit(
                    std::slice::from_ref(array),
                    values,
                    next_value,
                    operations,
                )?
            }
            LoweredScalarEffect::CallUnit(call) => {
                let types = values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>();
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| {
                        validate_direct_parameter_types(argument, &types)?;
                        if !matches!(argument, LoweredDirectExpression::Parameter { .. }) {
                            return unsupported("Unit call requires completed scalar operands");
                        }
                        let id = emit_direct_expression(argument, values, next_value, operations);
                        Ok(ValueDeclaration {
                            qualifications: Default::default(),
                            id,
                            scalar_type: argument.scalar_type(),
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let id = operations.allocate();
                operations.record_source_call_with_values(
                    call.source_coordinate,
                    None,
                    id,
                    call.target_state,
                    values,
                )?;
                operations.push(Operation {
                    static_reach_binding: None,
                    id,
                    result: OperationResult::Unit,
                    kind: OperationKind::CallUnit {
                        callee: lookup_machine_id(calls.machine_ids, call.target_machine)?,
                        arguments: arguments.iter().map(|argument| argument.id).collect(),
                        structural_arguments: call.structural_arguments.clone(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: lower_checked_crash_route_buckets(
                            &call.crash_routes,
                            &arguments,
                        )?,
                    },
                });
            }
        }
    }
    Ok(())
}
