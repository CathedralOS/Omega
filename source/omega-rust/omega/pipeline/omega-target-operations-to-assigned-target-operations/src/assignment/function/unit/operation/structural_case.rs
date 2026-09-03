//! Physical assignment for one bounded closed-sum inspection.

use std::collections::BTreeMap;

use super::{
    AssignedStructuralHome, AssignedUnitOperation, AssignedUnitScalarHome, AssignmentError,
    MachineId, PlaceId, TargetUnitOperation, ValueId,
};
use omega_assigned_target_operations::{
    AssignedUnitStructuralCasePayload, AssignedUnitStructuralCaseSuccessor,
};

pub(super) fn assign(
    machine: MachineId,
    body: &omega_target_operations::TargetUnitBody,
    operation: &TargetUnitOperation,
    preceding_operations: &[TargetUnitOperation],
    assigned_scalar_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    assigned_structural_homes: &BTreeMap<PlaceId, AssignedStructuralHome>,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::UnitCallCustodyMismatch {
        machine,
        operation: operation_identity(operation),
    };
    let TargetUnitOperation::StructuralCase { source, cases } = operation else {
        unreachable!("structural-case assignment receives only structural cases")
    };
    let assigned_source = assigned_structural_homes
        .get(&source.result.place)
        .filter(|home| home.requirement == *source)
        .cloned()
        .ok_or_else(invalid)?;
    if cases.len() != 2
        || cases.iter().enumerate().any(|(tag, case)| {
            case.case_tag != i32::try_from(tag).unwrap_or(-1)
                || usize::try_from(case.operation_ordinal)
                    .ok()
                    .is_none_or(|ordinal| ordinal <= preceding_operations.len())
        })
        || preceding_operations
            .iter()
            .filter(|candidate| {
                matches!(candidate, TargetUnitOperation::BoundarySettlement {
                psi_operation,
                result: omega_target_operations::TargetBoundaryResult::Structural(candidate),
                ..
            } if *psi_operation == source.defining_operation && candidate == source)
            })
            .count()
            != 1
    {
        return Err(invalid());
    }
    let declaration = body
        .structural_types
        .iter()
        .find(|declaration| declaration.id == source.result.structural_type)
        .ok_or_else(invalid)?;
    let psi_terminal::StructuralTypeShape::Sum {
        cases: declared_cases,
    } = &declaration.shape
    else {
        return Err(invalid());
    };
    if declared_cases.len() != cases.len() || source.layout.cases.len() != cases.len() {
        return Err(invalid());
    }

    let mut assigned_cases = Vec::with_capacity(cases.len());
    for (case_index, (case, declared_case)) in cases.iter().zip(declared_cases).enumerate() {
        if case.case != declared_case.id {
            return Err(invalid());
        }
        let relevant_fields = declared_case
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        let layout_fields = &source.layout.cases[case_index].fields;
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for payload in &case.payloads {
            let field_index = relevant_fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or_else(invalid)?;
            let layout = layout_fields.get(field_index).ok_or_else(invalid)?;
            if u32::from(layout.byte_offset) != payload.field_byte_offset
                || layout.shape != payload.home.shape
                || payload.home.defining_operation != source.defining_operation
            {
                return Err(invalid());
            }
            let home = AssignedUnitScalarHome {
                defining_operation: payload.home.defining_operation,
                source_value: payload.home.source_value,
                scalar_type: payload.home.scalar_type,
                shape: payload.home.shape,
                byte_offset: assigned_source
                    .byte_offset
                    .checked_add(payload.field_byte_offset)
                    .ok_or_else(invalid)?,
            };
            if assigned_scalar_homes
                .insert(home.source_value, home)
                .is_some()
            {
                return Err(invalid());
            }
            payloads.push(AssignedUnitStructuralCasePayload {
                field: payload.field,
                field_byte_offset: payload.field_byte_offset,
                home,
            });
        }
        assigned_cases.push(AssignedUnitStructuralCaseSuccessor {
            psi_edge: case.psi_edge,
            case: case.case,
            case_tag: case.case_tag,
            operation_ordinal: case.operation_ordinal,
            nominal_return_edge: case.nominal_return_edge,
            payloads,
        });
    }
    Ok(AssignedUnitOperation::StructuralCase {
        source: assigned_source,
        cases: assigned_cases,
    })
}

fn operation_identity(operation: &TargetUnitOperation) -> psi_core::OperationId {
    let TargetUnitOperation::StructuralCase { source, .. } = operation else {
        unreachable!()
    };
    source.defining_operation
}
