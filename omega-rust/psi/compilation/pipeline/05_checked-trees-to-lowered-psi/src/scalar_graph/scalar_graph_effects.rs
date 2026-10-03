//! Non-scalar graph effects consume completed operands without adding value slots.
use super::{
    LoweringError, Operation, OperationKind, OperationResult, ValueDeclaration,
    emit_direct_expression, lookup_machine_id, lower_checked_crash_route_buckets, unsupported,
    validate_direct_parameter_types,
};
use crate::direct_calls::{DirectCall, DirectCallResult};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::view_subslice;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarEffect;
use crate::terminal_identities::{allocate_dense, obligation_id};

pub(crate) fn emit(
    effects: &[LoweredScalarEffect],
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<(), LoweringError> {
    for effect in effects {
        match effect {
            LoweredScalarEffect::CopyStructuralValue {
                source,
                place,
                structural_type,
            } => {
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: *place,
                        structural_type: *structural_type,
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::StructuralLeafCopy {
                        source: *source,
                        path: Vec::new(),
                    },
                });
            }
            LoweredScalarEffect::StoreScalarField {
                destination,
                path,
                field,
                value_position,
                scalar_type,
                requires_evidence,
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
                        range_obligation: if *requires_evidence {
                            Some(obligation_id(allocate_dense(
                                &mut calls.next_obligation_identity,
                            )?))
                        } else {
                            None
                        },
                    },
                });
            }
            LoweredScalarEffect::EstablishRecord(record) => {
                crate::scalar_graph::scalar_graph_lowering::structural_values::emit(
                    record, values, operations, calls,
                )?;
            }
            LoweredScalarEffect::EstablishZeroedStructural {
                place,
                structural_type,
                multiplicity,
            } => {
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Structural(super::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: *place,
                        structural_type: *structural_type,
                        multiplicity: *multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishZeroedStructural {},
                });
            }
            LoweredScalarEffect::EstablishScalarCase(case) => {
                crate::scalar_graph::scalar_computations::cases::emit(
                    case, values, next_value, operations, calls,
                )?
            }
            LoweredScalarEffect::EstablishByteSequenceLiteral {
                place,
                structural_type: _,
                bytes,
            } => {
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Unit,
                    kind: OperationKind::EstablishByteSequenceLiteral {
                        destination: *place,
                        bytes: bytes.clone(),
                        qualifications: Vec::new(),
                    },
                });
            }
            LoweredScalarEffect::EstablishScalarArray(array) => {
                crate::scalar_graph::scalar_computations::arrays::emit(
                    std::slice::from_ref(array),
                    values,
                    next_value,
                    operations,
                )?
            }
            LoweredScalarEffect::EstablishElementView(view) => {
                crate::unit::attached_unit::view_ranges::establish(view, operations);
            }
            LoweredScalarEffect::ByteSequenceSubslice {
                source,
                access,
                start,
                end,
                place,
                structural_type,
            } => {
                view_subslice::emit(
                    view_subslice::ViewFamily::Bytes,
                    *source,
                    *access,
                    Some(start),
                    end.as_ref(),
                    *place,
                    *structural_type,
                    values,
                    next_value,
                    operations,
                )?;
            }
            LoweredScalarEffect::ElementViewSubslice {
                source,
                access,
                start,
                end,
                place,
                structural_type,
            } => {
                view_subslice::emit(
                    view_subslice::ViewFamily::Elements,
                    *source,
                    *access,
                    Some(start),
                    end.as_ref(),
                    *place,
                    *structural_type,
                    values,
                    next_value,
                    operations,
                )?;
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
                let erased_proof_arguments = call
                    .erased_proof_arguments
                    .iter()
                    .map(|term| {
                        crate::scalar_graph::scalar_contracts::lowered_proof_term(
                            term, values, erased,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Unit,
                    kind: DirectCall {
                        callee: lookup_machine_id(calls.machine_ids, call.target_machine)?,
                        arguments: arguments.iter().map(|argument| argument.id).collect(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments,
                        structural_arguments: call.structural_arguments.clone(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: lower_checked_crash_route_buckets(
                            &call.crash_routes,
                            &arguments,
                        )?,
                        result: DirectCallResult::Unit,
                    }
                    .into_operation_kind(),
                });
            }
        }
    }
    Ok(())
}
