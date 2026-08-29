use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
) -> Result<AssignedOperation, AssignmentError> {
    Ok(match operation {
        TargetOperation::UnitBody(body) => {
            let operations = body
                .operations
                .iter()
                .map(|operation| {
                    Ok(match operation {
                        TargetUnitOperation::EstablishByteSequenceLiteral {
                            psi_operation,
                            place,
                            structural_type,
                            bytes,
                        } => AssignedUnitOperation::EstablishByteSequenceLiteral {
                            psi_operation: *psi_operation,
                            place: place.clone(),
                            structural_type: structural_type.clone(),
                            bytes: bytes.clone(),
                        },
                        TargetUnitOperation::IntegerConstant {
                            psi_operation,
                            result,
                            scalar_type,
                            value,
                        } => AssignedUnitOperation::IntegerConstant {
                            psi_operation: *psi_operation,
                            result: *result,
                            scalar_type: *scalar_type,
                            value: *value,
                        },
                        TargetUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation,
                            place,
                            structural_type,
                        } => AssignedUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation: *psi_operation,
                            place: place.clone(),
                            structural_type: structural_type.clone(),
                        },
                        TargetUnitOperation::Call {
                            psi_operation,
                            callee,
                            arguments,
                            claim_transfers,
                        } => AssignedUnitOperation::Call {
                            psi_operation: *psi_operation,
                            callee: *callee,
                            result: None,
                            copies: arguments
                                .iter()
                                .map(|argument| AssignedAggregateCopy {
                                    place: argument.place,
                                    access: argument.access,
                                    path: argument.path.clone(),
                                    root_structural_type: argument.root_structural_type,
                                    structural_type: argument.structural_type,
                                    shape: argument.shape,
                                    source_byte_offset: argument.source_byte_offset,
                                    fixed_array_length: argument.fixed_array_length,
                                    element_stride: argument.element_stride,
                                    source: argument.source.clone(),
                                    destination: argument.destination.clone(),
                                })
                                .collect(),
                            claim_transfers: claim_transfers.clone(),
                        },
                        TargetUnitOperation::InstalledProviderCall {
                            psi_operation,
                            boundary,
                            ..
                        } => {
                            return Err(
                                AssignmentError::InstalledProviderCallRequiresOptimizedLane {
                                    machine: function.machine,
                                    operation: *psi_operation,
                                    boundary: *boundary,
                                },
                            );
                        }
                        TargetUnitOperation::PortWrite {
                            psi_operation,
                            service,
                            port,
                            value,
                        } => AssignedUnitOperation::PortWrite {
                            psi_operation: *psi_operation,
                            service: *service,
                            port: *port,
                            value: *value,
                        },
                        TargetUnitOperation::BoundarySettlement {
                            psi_operation,
                            boundary,
                            provider_execution,
                            realization,
                            scalar_arguments,
                            arguments,
                            byte_sequence_arguments,
                            completion_claim_sources,
                            completion_receipts,
                        } => AssignedUnitOperation::BoundarySettlement {
                            psi_operation: *psi_operation,
                            boundary: *boundary,
                            provider_execution: *provider_execution,
                            realization: *realization,
                            scalar_arguments: scalar_arguments.clone(),
                            arguments: arguments.clone(),
                            byte_sequence_arguments: byte_sequence_arguments.clone(),
                            completion_claim_sources: completion_claim_sources.clone(),
                            completion_receipts: completion_receipts.clone(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge,
                            cleanup_actions,
                        } => AssignedUnitOperation::Return {
                            psi_edge: *psi_edge,
                            cleanup_actions: cleanup_actions.clone(),
                        },
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            AssignedOperation::UnitBody(AssignedUnitBody {
                structural_types: body.structural_types.clone(),
                call_plan: body.call_plan.clone(),
                parameters: body.parameters.clone(),
                operations,
            })
        }
        _ => unreachable!("Unit assignment receives a Unit body"),
    })
}
