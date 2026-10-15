//! Primitive-local, structural-field and byte-sequence operations projected
//! to the legalized instruction kind that realizes each.

use super::super::{AbstractOperationPlan, Error, PsiOptimizationUnit};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperation;
use legalized_operations::LegalizedScalarInstructionKind;
use semantic_vocabulary::ScalarType;

pub(super) fn project_structural_case_membership(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralCaseMembership {
        psi_operation,
        source,
        path,
        case,
        result,
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_case_membership")
    };
    let kind = {
        if result.scalar_type != ScalarType::Boolean {
            return Err(Error::custody());
        }
        let (case_tag, tag_byte_offset, elements) =
            scalar_graph_input::structural_case::membership_layout(
                optimized, *source, path, *case, plan,
            )?;
        // A runtime element joins its selector and certificate, as a
        // primitive read's does.
        let indices = crate::legalization::runtime_indices::operands(
            optimized,
            unit,
            *psi_operation,
            &elements,
        )?;
        LegalizedScalarInstructionKind::StructuralCaseMembership {
            source: *source,
            path: path.clone(),
            case: *case,
            case_tag,
            tag_byte_offset,
            indices,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_leaf_copy(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralLeafCopy {
        psi_operation,
        result,
        source,
        path,
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_leaf_copy")
    };
    let kind = {
        let (byte_offset, shape, elements) = scalar_graph_input::structural_case::leaf_copy_layout(
            optimized,
            *psi_operation,
            *source,
            path,
            result,
            plan,
        )?;
        let indices = crate::legalization::runtime_indices::operands(
            optimized,
            unit,
            *psi_operation,
            &elements,
        )?;
        LegalizedScalarInstructionKind::StructuralLeafCopy {
            result: result.clone(),
            source: *source,
            path: path.clone(),
            byte_offset,
            shape,
            indices,
        }
    };
    Ok(kind)
}

/// Window byte movement consumes the target producer's selected storage.
/// Result/value declarations supply only the copy width; no restoration or
/// loan permission is replayed during instruction construction.
pub(super) fn project_borrowed_window(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    targeted: &target_operations::TargetUnitOperation,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    match (&node.operation, targeted) {
        (
            AbstractOperation::MoveStructuralField {
                psi_operation,
                result,
                source,
                ..
            },
            target_operations::TargetUnitOperation::StructuralLeafCopy {
                path,
                byte_offset,
                result_home,
                ..
            },
        ) => {
            let abstract_operations::StructuralCopyPath::Ordinary(selected) = path else {
                return Err(Error::custody());
            };
            Ok(LegalizedScalarInstructionKind::StructuralLeafCopy {
                result: result.clone(),
                source: source.place(),
                path: path.clone(),
                byte_offset: *byte_offset,
                shape: result_home.layout.shape(),
                indices: crate::legalization::runtime_indices::projection(
                    optimized,
                    unit,
                    *psi_operation,
                    source.structural_type(),
                    selected,
                    &plan.structural_types,
                )?,
            })
        }
        (
            AbstractOperation::StoreStructuralField { psi_operation, .. },
            target_operations::TargetUnitOperation::StoreStructuralField {
                destination,
                path,
                field,
                value,
                byte_offset,
                ..
            },
        ) => {
            let value_type =
                scalar_graph_input::primitive_locals::read_root_type(optimized, *value)
                    .ok_or(Error::custody())?;
            let shape = crate::structural_inputs::structural_reference_input::shape(
                value_type,
                &plan.structural_types,
            )
            .ok_or(Error::custody())?;
            Ok(LegalizedScalarInstructionKind::StoreStructuralField {
                destination: destination.clone(),
                path: path.clone(),
                field: *field,
                value: *value,
                byte_offset: *byte_offset,
                shape,
                indices: crate::legalization::runtime_indices::projection(
                    optimized,
                    unit,
                    *psi_operation,
                    destination.structural_type(),
                    path,
                    &plan.structural_types,
                )?,
            })
        }
        _ => Err(Error::custody()),
    }
}

/// One primitive observation. A projection through runtime elements joins
/// each to its selector and certificate over the original declared root,
/// whether supplied by a parameter or an established aggregate home.
pub(super) fn project_primitive_scalar_read(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::PrimitiveScalarRead {
        psi_operation,
        result,
        source,
        path,
    } = &node.operation
    else {
        unreachable!("dispatched project_primitive_scalar_read")
    };
    let indices = if terminal_psi::is_static_structural_path(path) {
        Vec::new()
    } else {
        let root_type = scalar_graph_input::primitive_locals::read_root_type(optimized, *source)
            .ok_or(Error::custody())?;
        let geometry =
            crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                root_type,
                path,
                result.scalar_type,
                &unit.structural_types,
            )
            .ok_or(Error::custody())?;
        crate::legalization::runtime_indices::operands(
            optimized,
            unit,
            *psi_operation,
            &geometry.elements,
        )?
    };
    Ok(LegalizedScalarInstructionKind::PrimitiveScalarRead {
        source: *source,
        path: path.clone(),
        indices,
    })
}

pub(super) fn project_write_only_primitive_store(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        path,
        value,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_write_only_primitive_store")
    };
    let kind = {
        let geometry = crate::structural_inputs::structural_reference_input::primitive_store(
            destination,
            path,
            value.scalar_type,
            &unit.structural_types,
        )
        .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination: destination.clone(),
            path: path.clone(),
            value: *value,
            byte_offset: geometry.byte_offset,
            byte_size: u8::try_from(geometry.byte_size).map_err(|_| Error::custody())?,
            indices: crate::legalization::runtime_indices::operands(
                optimized,
                unit,
                *psi_operation,
                &geometry.elements,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_scalar_field_store(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    targeted: &target_operations::TargetUnitOperation,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        field,
        value,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_scalar_field_store")
    };
    let kind = {
        let target_operations::TargetUnitOperation::StructuralScalarFieldStore {
            destination,
            path,
            field_byte_offset,
            ..
        } = targeted
        else {
            return Err(Error::custody());
        };
        let (_, byte_size, elements) = crate::structural_inputs::structural_reference_input::store(
            destination.structural_type(),
            path,
            *field,
            value.scalar_type,
            &unit.structural_types,
        )
        .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination: destination.clone(),
            path: path.clone(),
            field: *field,
            value: *value,
            byte_offset: *field_byte_offset,
            byte_size,
            indices: crate::legalization::runtime_indices::operands(
                optimized,
                unit,
                *psi_operation,
                &elements,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_byte_sequence_subslice(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ByteSequenceSubslice {
        psi_operation,
        result,
        source,
        start,
        end,
        length,
        obligation,
    } = &node.operation
    else {
        unreachable!("dispatched project_byte_sequence_subslice")
    };
    let kind = {
        LegalizedScalarInstructionKind::ByteSequenceSubslice {
            result: result.clone(),
            source: source.clone(),
            indices: crate::legalization::runtime_indices::operands(
                optimized,
                unit,
                *psi_operation,
                &scalar_graph_input::element_views::descriptor_geometry(
                    optimized,
                    source,
                    &unit.structural_types,
                )
                .ok_or(LegalizationError::custody())?
                .elements,
            )?,
            start: *start,
            end: *end,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_byte_sequence_field_byte_store(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    targeted: &target_operations::TargetUnitOperation,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralByteSequenceFieldByteStore {
        psi_operation,
        field,
        index,
        value,
        length,
        obligation,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_byte_sequence_field_byte_store")
    };
    let kind = {
        let target_operations::TargetUnitOperation::StructuralByteSequenceFieldByteStore {
            destination,
            ..
        } = targeted
        else {
            return Err(Error::custody());
        };
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
            destination: destination.clone(),
            field: *field,
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_byte_sequence_field_read(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralByteSequenceFieldRead {
        psi_operation,
        field,
        index,
        length,
        obligation,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_byte_sequence_field_read")
    };
    let source = crate::legalization::scalar_graph_input::structural_fields::byte_read(
        optimized,
        &node.operation,
        &unit.structural_types,
    )
    .ok_or(Error::custody())?;
    Ok(
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
            indices: scalar_graph_input::structural_fields::byte_index_operands(
                optimized,
                unit,
                *psi_operation,
                &source,
                *field,
            )?,
            source,
            field: *field,
            index: *index,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        },
    )
}

pub(super) fn project_structural_byte_sequence_field_store(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    targeted: &target_operations::TargetUnitOperation,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralByteSequenceFieldStore {
        psi_operation,
        field,
        source,
        length,
        obligation,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_byte_sequence_field_store")
    };
    let kind = {
        let target_operations::TargetUnitOperation::StructuralByteSequenceFieldStore {
            destination,
            ..
        } = targeted
        else {
            return Err(Error::custody());
        };
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
            destination: destination.clone(),
            field: *field,
            source: *source,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_byte_sequence_write(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ByteSequenceWrite {
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
    } = &node.operation
    else {
        unreachable!("dispatched project_byte_sequence_write")
    };
    let kind = {
        LegalizedScalarInstructionKind::ByteSequenceWrite {
            destination: *destination,
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_byte_sequence_read(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ByteSequenceRead {
        psi_operation,
        source,
        index,
        length,
        obligation,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_byte_sequence_read")
    };
    let kind = {
        LegalizedScalarInstructionKind::ByteSequenceRead {
            source: *source,
            index: *index,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_element_view_write(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ElementViewWrite {
        path,
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
    } = &node.operation
    else {
        unreachable!("dispatched project_element_view_write")
    };
    let kind = {
        LegalizedScalarInstructionKind::ElementViewWrite {
            path: path.clone(),
            destination: *destination,
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        }
    };
    Ok(kind)
}

pub(super) fn project_element_view_read(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ElementViewRead {
        psi_operation,
        source,
        index,
        length,
        obligation,
        path,
        result,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_element_view_read")
    };
    let kind = {
        LegalizedScalarInstructionKind::ElementViewRead {
            source: *source,
            index: *index,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
            path: path.clone(),
            indices: {
                let element = scalar_graph_input::byte_views::element_type(
                    optimized,
                    *source,
                    &unit.structural_types,
                )
                .ok_or(Error::custody())?;
                let (_, _, elements) =
                    crate::structural_inputs::structural_reference_input::element_leaf_projection(
                        element,
                        path,
                        result.scalar_type,
                        &unit.structural_types,
                    )
                    .ok_or(Error::custody())?;
                crate::legalization::runtime_indices::operands(
                    optimized,
                    unit,
                    *psi_operation,
                    &elements,
                )?
            },
        }
    };
    Ok(kind)
}

pub(super) fn project_element_view_subslice(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ElementViewSubslice {
        psi_operation,
        result,
        source,
        start,
        end,
        length,
        obligation,
    } = &node.operation
    else {
        unreachable!("dispatched project_element_view_subslice")
    };
    let kind = {
        LegalizedScalarInstructionKind::ElementViewSubslice {
            result: result.clone(),
            source: source.clone(),
            indices: crate::legalization::runtime_indices::operands(
                optimized,
                unit,
                *psi_operation,
                &scalar_graph_input::element_views::descriptor_geometry(
                    optimized,
                    source,
                    &unit.structural_types,
                )
                .ok_or(LegalizationError::custody())?
                .elements,
            )?,
            start: *start,
            end: *end,
            length: *length,
            obligation: *obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *obligation,
            )?,
        }
    };
    Ok(kind)
}

/// Retain both exact observations and the accepted sum bound. Selection chooses
/// activation storage only after rejoining the result's declared capacity.
pub(super) fn project_byte_sequence_concatenation(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::EstablishByteSequenceConcatenation {
        psi_operation,
        result,
        left,
        right,
        capacity_obligation,
    } = &node.operation
    else {
        return Err(Error::custody());
    };
    Ok(
        LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
            result: result.clone(),
            left: left.clone(),
            right: right.clone(),
            capacity_obligation: *capacity_obligation,
            accepted_fact: obligation_fact_identity(
                unit,
                optimized.machine,
                *psi_operation,
                *capacity_obligation,
            )?,
        },
    )
}

pub(super) fn project_byte_sequence_store(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    targeted: &target_operations::TargetUnitOperation,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ByteSequenceStore {
        psi_operation,
        destination,
        source,
        length,
        obligation,
    } = &node.operation
    else {
        return Err(Error::custody());
    };
    let target_operations::TargetUnitOperation::ByteSequenceStore { .. } = targeted else {
        return Err(Error::custody());
    };
    Ok(LegalizedScalarInstructionKind::ByteSequenceStore {
        destination: *destination,
        source: *source,
        length: *length,
        obligation: *obligation,
        accepted_fact: obligation_fact_identity(
            unit,
            optimized.machine,
            *psi_operation,
            *obligation,
        )?,
    })
}

/// The obligation fact `obligation` carries on `machine`'s operation
/// `psi_operation`, or the custody error when the unit recorded none.
fn obligation_fact_identity(
    unit: &PsiOptimizationUnit,
    machine: semantic_vocabulary::MachineId,
    operation: semantic_vocabulary::OperationId,
    obligation: semantic_vocabulary::ObligationId,
) -> Result<optimization_core::AcceptedObligationFactIdentity, Error> {
    Ok(accepted_obligation_fact(unit, machine, operation, obligation)?.identity)
}

fn accepted_obligation_fact<'unit>(
    unit: &'unit PsiOptimizationUnit,
    machine: semantic_vocabulary::MachineId,
    operation: semantic_vocabulary::OperationId,
    obligation: semantic_vocabulary::ObligationId,
) -> Result<&'unit optimization_unit::AcceptedObligationFact, Error> {
    unit.accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == machine
                && fact.owner == optimization_unit::PsiProvenance::Operation(operation)
                && fact.obligation == obligation
        })
        .ok_or(Error::custody())
}
