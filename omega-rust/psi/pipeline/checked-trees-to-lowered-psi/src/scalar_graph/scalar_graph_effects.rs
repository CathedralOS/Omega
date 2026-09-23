//! Non-scalar graph effects consume completed operands without adding value slots.
use super::{
    LoweringError, Operation, OperationKind, OperationResult, PlaceId, PrimitiveType,
    StructuralMultiplicity, ValueDeclaration, direct_expression_contains_short_circuit,
    emit_direct_expression, lookup_machine_id, lower_checked_crash_route_buckets, obligation_id,
    terminal_scalar_type, unsupported, validate_direct_parameter_types,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, emit_byte_length, emit_element_length,
};
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarEffect;
use semantic_vocabulary::{StructuralTypeId, ValueId};

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
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Unit,
                    kind: OperationKind::StructuralScalarFieldStore {
                        destination: *destination,
                        path: path.clone(),
                        field: *field,
                        value: value.id,
                        range_obligation: None,
                    },
                });
            }
            LoweredScalarEffect::EstablishRecord(record) => {
                crate::scalar_graph::scalar_graph_lowering::structural_values::emit(
                    record, values, operations, calls,
                )?;
            }
            LoweredScalarEffect::EstablishScalarCase(case) => {
                crate::scalar_graph::scalar_computations::cases::emit(
                    case, values, next_value, operations,
                )?
            }
            LoweredScalarEffect::EstablishScalarArray(array) => {
                crate::scalar_graph::scalar_computations::arrays::emit(
                    std::slice::from_ref(array),
                    values,
                    next_value,
                    operations,
                )?
            }
            LoweredScalarEffect::ByteSequenceSubslice {
                source,
                start,
                end,
                place,
                structural_type,
            } => emit_subslice(
                source,
                start,
                end.as_ref(),
                *place,
                *structural_type,
                false,
                values,
                next_value,
                operations,
            )?,
            LoweredScalarEffect::ElementViewSubslice {
                source,
                start,
                end,
                place,
                structural_type,
            } => emit_subslice(
                source,
                start,
                end.as_ref(),
                *place,
                *structural_type,
                true,
                values,
                next_value,
                operations,
            )?,
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
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Unit,
                    kind: OperationKind::CallUnit {
                        callee: lookup_machine_id(calls.machine_ids, call.target_machine)?,
                        arguments: arguments.iter().map(|argument| argument.id).collect(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: call.erased_proof_arguments.clone(),
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

/// Derive one borrowed window inside the edge's operation stream: endpoints
/// are completed branch-free u64 operands, the length observation must
/// directly name the source, and the obligation identity follows the op.
#[allow(clippy::too_many_arguments)]
fn emit_subslice(
    source: &PlaceId,
    start: &LoweredDirectExpression,
    end: Option<&LoweredDirectExpression>,
    place: PlaceId,
    structural_type: StructuralTypeId,
    element: bool,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let count_type = terminal_scalar_type(PrimitiveType::U64)?;
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    let mut endpoint = |expression: &LoweredDirectExpression| -> Result<ValueId, LoweringError> {
        if expression.scalar_type() != count_type
            || direct_expression_contains_short_circuit(expression)
        {
            return unsupported("scalar subslice endpoint needs a branch-free u64 value");
        }
        validate_direct_parameter_types(expression, &types)?;
        Ok(emit_direct_expression(
            expression, values, next_value, operations,
        ))
    };
    let start = endpoint(start)?;
    let end = match end {
        Some(end) => endpoint(end)?,
        None => {
            if element {
                operations
                    .element_lengths
                    .iter()
                    .rev()
                    .find_map(|(candidate, value)| (*candidate == *source).then_some(*value))
                    .unwrap_or_else(|| emit_element_length(*source, next_value, operations))
            } else {
                operations
                    .byte_lengths
                    .iter()
                    .rev()
                    .find_map(|(candidate, value)| (*candidate == *source).then_some(*value))
                    .unwrap_or_else(|| emit_byte_length(*source, next_value, operations))
            }
        }
    };
    let length = if element {
        operations
            .element_lengths
            .iter()
            .rev()
            .find_map(|(candidate, value)| (*candidate == *source).then_some(*value))
            .unwrap_or_else(|| emit_element_length(*source, next_value, operations))
    } else {
        operations
            .byte_lengths
            .iter()
            .rev()
            .find_map(|(candidate, value)| (*candidate == *source).then_some(*value))
            .unwrap_or_else(|| emit_byte_length(*source, next_value, operations))
    };
    let producer = operations.allocate();
    let obligation = obligation_id(producer.get().checked_add(1).ok_or(
        LoweringError::Unsupported("scalar subslice obligation identity overflows"),
    )?);
    let kind = if element {
        OperationKind::ElementViewSubslice {
            source: *source,
            start,
            end,
            length,
            obligation,
        }
    } else {
        OperationKind::ByteSequenceSubslice {
            source: *source,
            start,
            end,
            length,
            obligation,
        }
    };
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind,
    });
    Ok(())
}
