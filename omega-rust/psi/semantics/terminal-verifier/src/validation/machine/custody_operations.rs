//! Operations registered by the structural custody and obligations their
//! kind owns.

use super::super::structural_operations::validate_unit_operation_static;
use super::super::{
    BTreeMap, BoundaryMachineResult, BoundaryStructuralResultDeclaration, IdRegistry, MachineId,
    ModuleError, OperationKind, OperationResult, ScalarType, StructuralMultiplicity,
    TerminalMachine, TerminalModule, insert_unique, insert_value,
};
use semantic_vocabulary::ValueId;
use terminal_psi::Operation;

/// Registers an operation whose kind owns structural custody or
/// obligations: dynamic Unit and scalar calls, structural calls, scalar
/// case and record establishment, field and byte-sequence stores, Unit
/// operations without a scalar result, boundary calls and byte-sequence
/// subslices. Reports whether the operation was one of those; otherwise it
/// carries a scalar result typed by its sibling.
pub(super) fn register_custody_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &Operation,
    registry: &mut IdRegistry,
    value_types: &mut BTreeMap<ValueId, ScalarType>,
) -> Result<bool, ModuleError> {
    if let OperationKind::CallDynamicUnit {
        requirement_obligations,
        ..
    }
    | OperationKind::CallDynamicParameterUnit {
        requirement_obligations,
        ..
    } = &operation.kind
    {
        if operation.result != terminal_psi::OperationResult::Unit {
            return Err(ModuleError::UnitOperationHasScalarResult(operation.id));
        }
        for obligation in requirement_obligations {
            insert_unique(
                &mut registry.obligations,
                *obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        return Ok(true);
    }
    if let OperationKind::CallDynamicScalar {
        requirement_obligations,
        ..
    }
    | OperationKind::CallDynamicParameterScalar {
        requirement_obligations,
        ..
    } = &operation.kind
    {
        let Some(result) = operation.result.scalar() else {
            return Err(ModuleError::ScalarOperationHasUnitResult(operation.id));
        };
        insert_value(
            value_types,
            &mut registry.values,
            result.id,
            result.scalar_type,
        )?;
        for obligation in requirement_obligations {
            insert_unique(
                &mut registry.obligations,
                *obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        return Ok(true);
    }
    if let OperationKind::CallStructuralScalar {
        requirement_obligations,
        ..
    } = &operation.kind
    {
        let Some(result) = operation.result.scalar() else {
            return Err(ModuleError::ScalarOperationHasUnitResult(operation.id));
        };
        insert_value(
            value_types,
            &mut registry.values,
            result.id,
            result.scalar_type,
        )?;
        validate_unit_operation_static(module, machine, machines, operation)?;
        for obligation in requirement_obligations {
            insert_unique(
                &mut registry.obligations,
                *obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        return Ok(true);
    }
    if let OperationKind::CallStructural {
        requirement_obligations,
        ..
    } = &operation.kind
    {
        validate_unit_operation_static(module, machine, machines, operation)?;
        for obligation in requirement_obligations {
            insert_unique(
                &mut registry.obligations,
                *obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        return Ok(true);
    }
    if let OperationKind::CallStructuralWithScalarArguments {
        requirement_obligations,
        ..
    } = &operation.kind
    {
        validate_unit_operation_static(module, machine, machines, operation)?;
        for obligation in requirement_obligations {
            insert_unique(
                &mut registry.obligations,
                *obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        return Ok(true);
    }
    if let OperationKind::EstablishScalarCase { fields, .. } = &operation.kind {
        super::super::scalar_case::fields(module, machine, operation)?;
        for obligation in fields.iter().filter_map(|field| field.range_obligation) {
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        return Ok(true);
    }
    if let OperationKind::EstablishRecord { fields } = &operation.kind {
        super::super::record::fields(module, machine, operation)?;
        for obligation in fields.iter().filter_map(|field| match field.value {
            terminal_psi::RecordFieldValue::Scalar {
                range_obligation, ..
            } => range_obligation,
            terminal_psi::RecordFieldValue::Structural(_) => None,
        }) {
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        return Ok(true);
    }
    if let OperationKind::StructuralScalarFieldStore {
        range_obligation: Some(obligation),
        ..
    } = operation.kind
    {
        insert_unique(
            &mut registry.obligations,
            obligation,
            ModuleError::DuplicateObligation,
        )?;
    }
    if matches!(
        operation.kind,
        OperationKind::EstablishRecord { .. }
            | OperationKind::EstablishReference { .. }
            | OperationKind::EstablishScalarArray { .. }
            | OperationKind::EstablishPrimitiveLocal { .. }
    ) {
        validate_unit_operation_static(module, machine, machines, operation)?;
        return Ok(true);
    }
    if matches!(operation.kind, OperationKind::MoveStructuralField { .. }) {
        super::super::borrowed_windows::validate_move_static(module, machine, operation)?;
        return Ok(true);
    }
    if matches!(
        operation.kind,
        OperationKind::CallUnit { .. }
            | OperationKind::ReleaseReference { .. }
            | OperationKind::WriteOnlyPrimitiveStore { .. }
            | OperationKind::WriteOnlyIndexedPrimitiveStore { .. }
            | OperationKind::StructuralScalarFieldStore { .. }
            | OperationKind::StoreStructuralField { .. }
            | OperationKind::StructuralByteSequenceFieldStore { .. }
            | OperationKind::StructuralByteSequenceFieldByteStore { .. }
            | OperationKind::ByteSequenceWrite { .. }
            | OperationKind::PortWrite { .. }
            | OperationKind::EstablishByteSequenceLiteral { .. }
            | OperationKind::EstablishTrivialAffineLocal { .. }
            | OperationKind::StoreDynamicDescriptor { .. }
    ) {
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(ModuleError::UnitOperationHasScalarResult(operation.id));
        }
        validate_unit_operation_static(module, machine, machines, operation)?;
        if let OperationKind::StructuralByteSequenceFieldStore { obligation, .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { obligation, .. }
        | OperationKind::WriteOnlyIndexedPrimitiveStore { obligation, .. }
        | OperationKind::ByteSequenceWrite { obligation, .. } = &operation.kind
        {
            insert_unique(
                &mut registry.obligations,
                *obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        if let OperationKind::CallUnit {
            requirement_obligations,
            ..
        } = &operation.kind
        {
            for obligation in requirement_obligations {
                insert_unique(
                    &mut registry.obligations,
                    *obligation,
                    ModuleError::DuplicateObligation,
                )?;
            }
        }
        return Ok(true);
    }
    if let OperationKind::BoundaryCall { boundary, .. } = &operation.kind {
        let boundary = module
            .boundary_machines
            .iter()
            .find(|candidate| candidate.id == *boundary)
            .ok_or(ModuleError::UnknownBoundaryCallTarget {
                operation: operation.id,
                boundary: *boundary,
            })?;
        let actual = match &operation.result {
            OperationResult::Unit => Some(BoundaryMachineResult::Unit),
            OperationResult::Scalar(result) => {
                Some(BoundaryMachineResult::Scalar(result.scalar_type))
            }
            OperationResult::Structural(result)
                if result.projected_qualifications.is_empty()
                    && (result.claims.is_empty()
                        || result.multiplicity == StructuralMultiplicity::Linear) =>
            {
                Some(BoundaryMachineResult::Structural(
                    BoundaryStructuralResultDeclaration {
                        structural_type: result.structural_type,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications.clone(),
                    },
                ))
            }
            OperationResult::Structural(_) => None,
        };
        if actual.as_ref() != Some(&boundary.result) {
            return Err(ModuleError::BoundaryCallResultMismatch {
                operation: operation.id,
                expected: boundary.result.clone(),
                actual,
            });
        }
        if let Some(result) = operation.result.scalar() {
            insert_value(
                value_types,
                &mut registry.values,
                result.id,
                result.scalar_type,
            )?;
        }
        validate_unit_operation_static(module, machine, machines, operation)?;
        return Ok(true);
    }
    if let OperationKind::ByteSequenceSubslice {
        source,
        length,
        obligation,
        ..
    } = operation.kind
    {
        super::super::byte_sequence_subslice::validate(module, machine, operation, source, length)?;
        insert_unique(
            &mut registry.obligations,
            obligation,
            ModuleError::DuplicateObligation,
        )?;
        return Ok(true);
    }
    Ok(false)
}
