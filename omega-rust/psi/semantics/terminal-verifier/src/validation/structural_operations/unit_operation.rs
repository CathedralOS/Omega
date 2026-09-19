//! Static validation of one Unit operation.
//!
//! `validate_unit_operation_static` first lets the reference and primitive
//! structural call checks claim the operation, then dispatches its kind:
//! short arms stay here and the long ones live in `unit_calls`,
//! `structural_calls`, `boundary_calls` and `local_establishments`.

mod boundary_calls;
mod local_establishments;
mod structural_calls;
mod unit_calls;

use crate::validation::structural_operations::claim_transfers::validate_service_reach;
use crate::validation::structural_operations::contract_places::validate_unit_call_contract_places;
use crate::validation::structural_operations::primitive_calls::validate_primitive_structural_call;
use crate::validation::structural_operations::structural_arguments::{
    StructuralArgumentSourcePolicy, validate_structural_arguments,
};
use crate::validation::{
    BTreeMap, MachineId, ModuleError, OperationKind, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralPlaceKind, TerminalMachine, TerminalModule, Terminator,
};

pub(crate) fn validate_unit_operation_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    if crate::validation::references::validate_call(module, machine, machines, operation)? {
        return Ok(());
    }
    if validate_primitive_structural_call(module, machine, machines, operation)? {
        return Ok(());
    }
    match &operation.kind {
        OperationKind::EstablishReference { source } => {
            crate::validation::references::validate_establishment(
                module, machine, operation, source,
            )?;
        }
        OperationKind::ReleaseReference { source } => {
            if crate::validation::references::carrier_type(module, machine, *source).is_none() {
                return Err(crate::validation::references::invalid(
                    machine,
                    "release source is not a reference carrier",
                ));
            }
        }
        OperationKind::EstablishScalarArray { .. } => {
            crate::validation::scalar_array::shape(module, machine, operation)?;
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
            crate::validation::primitive_storage::store_type(
                module,
                machine,
                operation.id,
                *destination,
                path,
            )?;
        }
        OperationKind::WriteOnlyIndexedPrimitiveStore {
            destination, path, ..
        } => {
            crate::validation::primitive_storage::indexed_store_shape(
                module,
                machine,
                operation.id,
                *destination,
                path,
            )?;
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            crate::validation::primitive_storage::validate_establishment(
                module, machine, operation,
            )?;
        }
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => {
            crate::validation::structural_scalar_fields::structural_scalar_field_store_type(
                module,
                machine,
                operation.id,
                *destination,
                path,
                *field,
            )?;
        }
        OperationKind::StoreStructuralField { .. } => {
            crate::validation::borrowed_windows::validate_store_static(module, machine, operation)?;
        }
        OperationKind::ByteSequenceWrite { .. } => {
            crate::validation::byte_sequence_write::validate(module, machine, operation)?;
        }
        OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
            crate::validation::structural_byte_sequence_fields::validate(
                module, machine, operation,
            )?;
        }
        OperationKind::StructuralByteSequenceFieldStore { .. } => {
            crate::validation::structural_byte_sequence_store::capacity(
                module, machine, operation,
            )?;
        }
        OperationKind::EstablishScalarCase { .. } => {
            crate::validation::scalar_case::fields(module, machine, operation)?;
        }
        OperationKind::EstablishRecord { .. } => {
            crate::validation::record::fields(module, machine, operation)?;
        }
        OperationKind::CallUnit { .. } => {
            unit_calls::validate_call_unit(module, machine, machines, operation)?
        }
        OperationKind::CallStructuralScalar { .. } => {
            structural_calls::validate_call_structural_scalar(module, machine, machines, operation)?
        }
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments: _,
            erased_arguments: _,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } if operation
            .result
            .structural()
            .is_none_or(|result| result.multiplicity != StructuralMultiplicity::Linear) =>
        {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            let Some(callee_result) = callee.result.structural() else {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            let Some(result) = operation.result.structural() else {
                return Err(ModuleError::StructuralCallResultMismatch(operation.id));
            };
            let [callee_parameter] = callee.structural_parameters.as_slice() else {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            let [argument] = structural_arguments.as_slice() else {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            let exact_return = matches!(callee.blocks.as_slice(), [block]
            if block.id == callee.entry
                && block.parameters.is_empty()
                && block.operations.is_empty()
                && matches!(
                    &block.terminator,
                    Terminator::ReturnStructural {
                        source,
                        returned_claims,
                        trivial_affine_discards,
                        ..
                    } if *source == callee_parameter.place
                        && returned_claims.is_empty()
                        && trivial_affine_discards.is_empty()
                ));
            if !callee.parameters.iter().all(|parameter| {
                matches!(
                    parameter.scalar_type,
                    ScalarType::Integer(integer)
                        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64)
                )
            }) || callee_parameter.position != 0
                || callee_parameter.is_self
                || callee_parameter.structural_type != callee_result.structural_type
                || callee_parameter.multiplicity != StructuralMultiplicity::Affine
                || callee_parameter.access != StructuralAccess::Owned
                || !callee_parameter.qualifications.is_empty()
                || !callee_parameter.projected_qualifications.is_empty()
                || !argument.path.is_empty()
                || argument.access != StructuralAccess::Owned
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != StructuralMultiplicity::Affine
                || result.multiplicity != callee_result.multiplicity
                || !crate::validation::structural_result_contracts::call_result_matches(
                    result,
                    callee_result,
                )
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !claim_transfers.is_empty()
                || !returned_claim_transfers.is_empty()
                || !requirement_obligations.is_empty()
                || !crash_continuations.is_empty()
                || !callee.entry_claims.is_empty()
                || !callee.content_entry_claims.is_empty()
                || !callee.content_identity_reshuffles.is_empty()
                || !callee.content_partition_compositions.is_empty()
                || !callee.published_service_ceiling.is_empty()
                || !callee.contract.requires.is_empty()
                || !callee.contract.ensures.is_empty()
                || !callee.contract.outcome_specific_ensures.is_empty()
                || !callee.contract.crash_routes.is_empty()
                || !crate::validation::structural_result_contracts::has_plain_owned_shape(
                    module,
                    callee_result.structural_type,
                )
                || !exact_return
            {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            let result_place = machine
                .structural_places
                .iter()
                .find(|place| place.id == result.place);
            if !matches!(
                result_place.map(|place| place.kind),
                Some(StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                }) if producer == operation.id && structural_type == result.structural_type
            ) {
                return Err(ModuleError::StructuralCallResultPlaceMismatch(operation.id));
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                false,
                StructuralArgumentSourcePolicy::ParametersOrAffineOperationResults,
            )?;
            validate_unit_call_contract_places(callee, operation.id)?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
        }
        OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. } => {
            structural_calls::validate_call_structural(module, machine, machines, operation)?
        }
        OperationKind::BoundaryCall { .. } => {
            boundary_calls::validate_boundary_call(module, machine, operation)?
        }
        OperationKind::PortWrite { .. } => {
            boundary_calls::validate_port_write(module, machine, operation)?
        }
        OperationKind::EstablishByteSequenceLiteral { .. } => {
            local_establishments::validate_establish_byte_sequence_literal(
                module, machine, operation,
            )?
        }
        OperationKind::EstablishTrivialAffineLocal { .. } => {
            local_establishments::validate_establish_trivial_affine_local(
                module, machine, operation,
            )?
        }
        OperationKind::StoreDynamicDescriptor { .. } => {
            // The dynamic-dispatch validator owns the exact descriptor,
            // selection, aggregate identity, field identity, and ordering.
        }
        _ => unreachable!("caller selects only structural/effect operations"),
    }
    Ok(())
}
