use super::{
    dynamic, dynamic_argument, foreign_call, installed_provider, scalar_call, structural_scalar,
};
use crate::assignment::shared::*;

mod boundary_settlement;
mod conditional;
mod ieee_float;

pub(super) fn assign(
    machine: MachineId,
    attachment: Option<psi_core::StructuralTypeId>,
    body: &omega_target_operations::TargetUnitBody,
    operation: &TargetUnitOperation,
    preceding_operations: &[TargetUnitOperation],
    preceding_assigned_operations: &[AssignedUnitOperation],
    target: NativeTarget,
    native_callback: Option<&omega_target_operations::TargetNativeCallbackArgument>,
    assigned_scalar_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    assigned_structural_homes: &mut BTreeMap<PlaceId, AssignedStructuralHome>,
    next_frame_home: &mut u32,
) -> Result<AssignedUnitOperation, AssignmentError> {
    Ok(match operation {
        TargetUnitOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes,
        } => AssignedUnitOperation::EstablishByteSequenceLiteral {
            psi_operation: *psi_operation,
            place: *place,
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
        TargetUnitOperation::StructuralScalarFieldStore {
            psi_operation,
            destination,
            path,
            field,
            destination_placement,
            field_byte_offset,
            source,
        } => structural_scalar::assign_field_store(
            machine,
            attachment,
            body,
            *psi_operation,
            destination,
            path,
            *field,
            destination_placement,
            *field_byte_offset,
            *source,
            preceding_operations,
        )?,
        TargetUnitOperation::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => AssignedUnitOperation::IeeeFloatConstant {
            psi_operation: *psi_operation,
            result: *result,
            value: *value,
        },
        TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd { .. } => {
            ieee_float::assign_fused_multiply_add(machine, operation, preceding_operations, target)?
        }
        TargetUnitOperation::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            structural_type,
        } => AssignedUnitOperation::EstablishTrivialAffineLocal {
            psi_operation: *psi_operation,
            place: *place,
            structural_type: structural_type.clone(),
        },
        TargetUnitOperation::EstablishAffineScalarRecord {
            psi_operation,
            result,
            field,
            value,
            shape,
        } => AssignedUnitOperation::EstablishAffineScalarRecord {
            psi_operation: *psi_operation,
            result: result.clone(),
            field: *field,
            value: *value,
            shape: *shape,
        },
        TargetUnitOperation::Call {
            psi_operation,
            callee,
            arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let invalid = || AssignmentError::UnitCallCustodyMismatch {
                machine,
                operation: *psi_operation,
            };
            if arguments.iter().any(|argument| {
                let parameter_source = body.parameters.iter().any(|parameter| {
                    parameter.place == argument.place
                        && parameter.structural_type == argument.root_structural_type
                        && parameter.shape == argument.source.shape
                        && parameter.placement == argument.source
                });
                let trivial_local_source = preceding_operations
                    .iter()
                    .filter_map(|preceding| match preceding {
                        TargetUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation,
                            place,
                            structural_type,
                        } if place.id == argument.place => {
                            Some((*psi_operation, place, structural_type))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let exact_trivial_local = matches!(trivial_local_source.as_slice(), [(establishment, place, structural_type)]
                    if argument.path.is_empty()
                        && argument.access == psi_terminal::StructuralAccess::Owned
                        && argument.root_structural_type == structural_type.id
                        && argument.structural_type == structural_type.id
                        && argument.shape == ValueShape::integer(0, 1)
                        && argument.source.shape == argument.shape
                        && argument.source.locations.is_empty()
                        && matches!(
                            place.kind,
                            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                                structural_type: local_type,
                                construction: None,
                                ..
                            } if local_type == structural_type.id
                        )
                        && matches!(
                            structural_type.shape,
                            psi_terminal::StructuralTypeShape::Record { ref fields }
                                if fields.is_empty()
                        )
                        && body.structural_types.iter().any(|candidate| candidate == *structural_type)
                        && !preceding_operations.iter().any(|preceding| match preceding {
                            TargetUnitOperation::Call { arguments, .. }
                            | TargetUnitOperation::StructuralScalarCall { arguments, .. } => {
                                arguments.iter().any(|candidate| {
                                    candidate.place == argument.place
                                        && candidate.path.is_empty()
                                        && candidate.access == psi_terminal::StructuralAccess::Owned
                                })
                            }
                            _ => false,
                        })
                        && *establishment != *psi_operation);
                let affine_scalar_record_source = preceding_operations
                    .iter()
                    .filter_map(|preceding| match preceding {
                        TargetUnitOperation::EstablishAffineScalarRecord {
                            psi_operation,
                            result,
                            field,
                            value,
                            shape,
                        } if result.place == argument.place => {
                            Some((*psi_operation, result, *field, *value, *shape))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let exact_affine_scalar_record = matches!(
                    affine_scalar_record_source.as_slice(),
                    [(establishment, result, _, _, shape)]
                        if argument.path.is_empty()
                            && argument.access == psi_terminal::StructuralAccess::Owned
                            && argument.root_structural_type == result.structural_type
                            && argument.structural_type == result.structural_type
                            && argument.shape == ValueShape::integer(8, 8)
                            && *shape == argument.shape
                            && argument.source.shape == argument.shape
                            && argument.source.locations.is_empty()
                            && result.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                            && !preceding_operations.iter().any(|preceding| match preceding {
                                TargetUnitOperation::Call { arguments, .. }
                                | TargetUnitOperation::StructuralScalarCall { arguments, .. } => {
                                    arguments.iter().any(|candidate| {
                                        candidate.place == argument.place
                                            && candidate.path.is_empty()
                                            && candidate.access == psi_terminal::StructuralAccess::Owned
                                    })
                                }
                                _ => false,
                            })
                            && *establishment != *psi_operation
                );
                !parameter_source && !exact_trivial_local && !exact_affine_scalar_record
            }) {
                return Err(invalid());
            }
            AssignedUnitOperation::Call {
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
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
            }
        }
        TargetUnitOperation::ScalarCall {
            psi_operation,
            callee,
            call_plan,
            result_home,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => scalar_call::assign(
            machine,
            *psi_operation,
            *callee,
            call_plan,
            *result_home,
            arguments,
            requirement_obligations,
            crash_continuations,
            preceding_operations,
            target,
            assigned_scalar_homes,
            next_frame_home,
        )?,
        TargetUnitOperation::StructuralScalarCall {
            psi_operation,
            result,
            callee,
            call_plan,
            scalar_arguments,
            arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => structural_scalar::assign_call(
            machine,
            attachment,
            body,
            target,
            *psi_operation,
            *result,
            *callee,
            call_plan,
            scalar_arguments,
            arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            preceding_operations,
            assigned_scalar_homes,
        )?,
        TargetUnitOperation::StructuralResultCall {
            psi_operation,
            result,
            callee,
            callee_result,
            call_plan,
            scalar_arguments,
            arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => structural_scalar::assign_result_call(
            machine,
            attachment,
            body,
            target,
            *psi_operation,
            result,
            *callee,
            callee_result,
            call_plan,
            scalar_arguments,
            arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            preceding_operations,
            assigned_scalar_homes,
        )?,
        TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
            psi_operation,
            result,
            callee,
            call_plan,
            result_home,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => dynamic_argument::assign(
            machine,
            body,
            target,
            *psi_operation,
            Some(*result),
            *callee,
            call_plan,
            Some(*result_home),
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            assigned_scalar_homes,
            next_frame_home,
        )?,
        TargetUnitOperation::StructuralUnitCallWithDynamicArguments {
            psi_operation,
            callee,
            call_plan,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => dynamic_argument::assign(
            machine,
            body,
            target,
            *psi_operation,
            None,
            *callee,
            call_plan,
            None,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            assigned_scalar_homes,
            next_frame_home,
        )?,
        TargetUnitOperation::StoreDynamicDescriptor {
            psi_operation,
            stored,
            source_argument,
        } => dynamic::assign_stored_descriptor(
            machine,
            target,
            *psi_operation,
            stored,
            source_argument,
            next_frame_home,
        )?,
        TargetUnitOperation::StoredDynamicScalarCall {
            psi_operation,
            result,
            dynamic_dispatch,
            call_plan,
            result_home,
            source_argument,
            requirement_obligations,
            crash_continuations,
        } => dynamic::assign_stored_scalar_call(
            machine,
            target,
            *psi_operation,
            *result,
            dynamic_dispatch,
            call_plan,
            *result_home,
            source_argument,
            requirement_obligations,
            crash_continuations,
            preceding_assigned_operations,
            assigned_scalar_homes,
            next_frame_home,
        )?,
        TargetUnitOperation::DynamicScalarCall {
            psi_operation,
            result,
            dynamic_dispatch,
            call_plan,
            result_home,
            initial_argument,
            rebound_argument,
            requirement_obligations,
            crash_continuations,
        } => dynamic::assign(
            machine,
            target,
            *psi_operation,
            *result,
            dynamic_dispatch,
            call_plan,
            *result_home,
            initial_argument,
            rebound_argument,
            requirement_obligations,
            crash_continuations,
            assigned_scalar_homes,
            next_frame_home,
        )?,
        TargetUnitOperation::DynamicUnitCall {
            psi_operation,
            dynamic_dispatch,
            call_plan,
            initial_argument,
            rebound_argument,
            requirement_obligations,
            crash_continuations,
        } => dynamic::assign_unit(
            machine,
            target,
            *psi_operation,
            dynamic_dispatch,
            call_plan,
            initial_argument,
            rebound_argument,
            requirement_obligations,
            crash_continuations,
            next_frame_home,
        )?,
        TargetUnitOperation::ConditionalIntegerEqual { .. }
        | TargetUnitOperation::ConditionalBoolean { .. }
        | TargetUnitOperation::ConditionalBooleanParameter { .. }
        | TargetUnitOperation::ConditionalDispatch { .. }
        | TargetUnitOperation::NonreturningTail { .. } => conditional::assign(
            operation,
            body,
            preceding_operations,
            target,
            assigned_scalar_homes,
        )?,
        TargetUnitOperation::InstalledProviderCall {
            psi_operation,
            boundary,
            provider,
            call_plan,
            scalar_arguments,
            source_arguments,
            arguments,
            claim_transfers,
            completion_claim_sources,
            completion_receipts,
        } => {
            if scalar_arguments.is_empty() {
                return Err(
                    AssignmentError::InstalledProviderCallRequiresOptimizedLane {
                        machine,
                        operation: *psi_operation,
                        boundary: *boundary,
                    },
                );
            }
            installed_provider::assign(
                machine,
                body,
                target,
                *psi_operation,
                *boundary,
                provider,
                call_plan,
                scalar_arguments,
                source_arguments,
                arguments,
                claim_transfers,
                completion_claim_sources,
                completion_receipts,
            )?
        }
        TargetUnitOperation::NormalizedForeignCall {
            psi_operation,
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            result_home,
        } => {
            let (scalar_arguments, result_home) = foreign_call::assign(
                *psi_operation,
                binding,
                target,
                scalar_arguments,
                *result_home,
                preceding_operations,
                native_callback,
                assigned_scalar_homes,
                next_frame_home,
            )?;
            AssignedUnitOperation::NormalizedForeignCall {
                psi_operation: *psi_operation,
                boundary: *boundary,
                provider_execution: *provider_execution,
                binding: binding.clone(),
                scalar_arguments,
                result_home,
            }
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
            result,
            execution,
            realization,
            scalar_arguments,
            runtime_scalar_arguments,
            arguments,
            byte_sequence_arguments,
            completion_claim_sources,
            completion_receipts,
        } => AssignedUnitOperation::BoundarySettlement {
            psi_operation: *psi_operation,
            boundary: *boundary,
            result: boundary_settlement::assign_result(
                *psi_operation,
                result,
                assigned_structural_homes,
                next_frame_home,
            )?,
            execution: *execution,
            realization: *realization,
            scalar_arguments: scalar_arguments.clone(),
            runtime_scalar_arguments: boundary_settlement::assign_runtime_scalar_arguments(
                target,
                runtime_scalar_arguments,
                preceding_operations,
                assigned_scalar_homes,
            )?,
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
}
