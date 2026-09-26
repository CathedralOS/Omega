//! Primitive-local, structural-field and byte-sequence operations projected
//! to the legalized instruction kind that realizes each.

use super::super::{AbstractOperationPlan, Error, PsiOptimizationUnit};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use crate::legalized_operations::LegalizedScalarInstructionKind;
use semantic_vocabulary::ScalarType;
use terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation;

pub(super) fn project_structural_case_membership(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralCaseMembership {
        source,
        path,
        case,
        result,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_case_membership")
    };
    let kind = {
        if result.scalar_type != ScalarType::Boolean {
            return Err(Error::custody());
        }
        let (case_tag, tag_byte_offset) = scalar_graph_input::structural_case::membership_layout(
            optimized, *source, path, *case, plan,
        )?;
        LegalizedScalarInstructionKind::StructuralCaseMembership {
            source: *source,
            path: path.clone(),
            case: *case,
            case_tag,
            tag_byte_offset,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_leaf_copy(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
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
            optimized, *source, path, result, plan,
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

/// A borrowed window's move is the leaf-copy byte copy over the spelled path
/// extended by the vacated field; its store is the inverse copy. Both
/// resolve one field extent independently of the target rows they rejoin.
pub(super) fn project_borrowed_window(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    match &node.operation {
        AbstractOperation::MoveStructuralField {
            result,
            source,
            path,
            field,
            ..
        } => {
            let extent = scalar_graph_input::borrowed_windows::moved(
                optimized, source, path, *field, result, plan,
            )?;
            Ok(LegalizedScalarInstructionKind::StructuralLeafCopy {
                result: result.clone(),
                source: source.place,
                path: extent.path,
                byte_offset: extent.byte_offset,
                shape: extent.shape,
                indices: Vec::new(),
            })
        }
        AbstractOperation::StoreStructuralField {
            destination,
            path,
            field,
            value,
            ..
        } => {
            let extent = scalar_graph_input::borrowed_windows::extent(
                optimized,
                destination,
                path,
                *field,
                plan,
            )?;
            if value.access != terminal_psi::StructuralAccess::Owned || !value.path.is_empty() {
                return Err(Error::custody());
            }
            Ok(LegalizedScalarInstructionKind::StoreStructuralField {
                destination: destination.clone(),
                path: path.clone(),
                field: *field,
                value: value.place,
                byte_offset: extent.byte_offset,
                shape: extent.shape,
            })
        }
        _ => unreachable!("dispatched project_borrowed_window"),
    }
}

/// One primitive observation. A projection through runtime elements joins
/// each to its selector and certificate; its root is then a structural
/// parameter, since a primitive local has no projection.
pub(super) fn project_primitive_scalar_read(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
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
        let root = optimized
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == *source)
            .ok_or(Error::custody())?;
        let (_, _, elements) =
            crate::structural_inputs::structural_reference_input::primitive_geometry(
                root.structural_type,
                path,
                result.scalar_type,
                &unit.structural_types,
            )
            .ok_or(Error::custody())?;
        crate::legalization::runtime_indices::operands(optimized, unit, *psi_operation, &elements)?
    };
    Ok(LegalizedScalarInstructionKind::PrimitiveScalarRead {
        source: *source,
        path: path.clone(),
        indices,
    })
}

pub(super) fn project_write_only_primitive_store(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        path,
        value,
    } = &node.operation
    else {
        unreachable!("dispatched project_write_only_primitive_store")
    };
    let kind = {
        let (byte_offset, byte_size, elements) =
            crate::structural_inputs::structural_reference_input::primitive_store(
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
            byte_offset,
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

pub(super) fn project_structural_scalar_field_store(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        value,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_structural_scalar_field_store")
    };
    let kind = {
        let (byte_offset, byte_size, elements) =
            crate::structural_inputs::structural_reference_input::store(
                destination.structural_type,
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
            byte_offset,
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
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
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
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::ByteSequenceSubslice {
            result: result.clone(),
            source: *source,
            start: *start,
            end: *end,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_byte_sequence_field_byte_store(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
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
        let destination = crate::legalization::scalar_graph_input::structural_fields::replacement(
            optimized,
            &node.operation,
            &unit.structural_types,
        )
        .ok_or(Error::custody())?;
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
            destination,
            field: *field,
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_byte_sequence_field_read(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
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
    let fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == optimized.machine
                && fact.operation == *psi_operation
                && fact.obligation == *obligation
        })
        .ok_or(Error::custody())?;
    Ok(
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
            source,
            field: *field,
            index: *index,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        },
    )
}

pub(super) fn project_structural_byte_sequence_field_store(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
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
        let destination = crate::legalization::scalar_graph_input::structural_fields::replacement(
            optimized,
            &node.operation,
            &unit.structural_types,
        )
        .ok_or(Error::custody())?;
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
            destination,
            field: *field,
            source: *source,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_byte_sequence_write(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
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
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::ByteSequenceWrite {
            destination: *destination,
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_byte_sequence_read(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
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
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::ByteSequenceRead {
            source: *source,
            index: *index,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_element_view_read(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::ElementViewRead {
        psi_operation,
        source,
        index,
        length,
        obligation,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_element_view_read")
    };
    let kind = {
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::ElementViewRead {
            source: *source,
            index: *index,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_element_view_subslice(
    node: &terminal_psi_to_abstract_operations::optimization_unit::OptimizationNode,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
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
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::ElementViewSubslice {
            result: result.clone(),
            source: *source,
            start: *start,
            end: *end,
            length: *length,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}
