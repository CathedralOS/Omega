//! Primitive-local, structural-field and byte-sequence operations projected
//! to the legalized instruction kind that realizes each.

use super::super::{
    AbstractOperation, AbstractOperationPlan, Error, LegalizedScalarInstructionKind,
    PsiOptimizationUnit,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use semantic_vocabulary::ScalarType;

pub(super) fn project_structural_case_membership(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
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
            return Err(Error::SourceCustodyMismatch);
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

pub(super) fn project_write_only_primitive_store(
    node: &optimization_unit::OptimizationNode,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::WriteOnlyPrimitiveStore {
        destination,
        path,
        value,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_write_only_primitive_store")
    };
    let kind = {
        let (byte_offset, byte_size) =
            crate::structural_inputs::structural_reference_input::primitive_store(
                destination,
                path,
                value.scalar_type,
                &unit.structural_types,
            )
            .ok_or(Error::SourceCustodyMismatch)?;
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination: destination.clone(),
            path: path.clone(),
            value: *value,
            byte_offset,
            byte_size,
        }
    };
    Ok(kind)
}

pub(super) fn project_write_only_indexed_primitive_store(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::WriteOnlyIndexedPrimitiveStore {
        psi_operation,
        destination,
        path,
        index,
        value,
        obligation,
    } = &node.operation
    else {
        unreachable!("dispatched project_write_only_indexed_primitive_store")
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
            .ok_or(Error::SourceCustodyMismatch)?;
        let (byte_offset, byte_size, extent) =
            crate::structural_inputs::structural_reference_input::indexed_primitive_store(
                destination,
                path,
                value.scalar_type,
                &unit.structural_types,
            )
            .ok_or(Error::SourceCustodyMismatch)?;
        LegalizedScalarInstructionKind::WriteOnlyIndexedPrimitiveStore {
            destination: destination.clone(),
            path: path.clone(),
            index: *index,
            value: *value,
            byte_offset,
            byte_size,
            extent,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_structural_scalar_field_store(
    node: &optimization_unit::OptimizationNode,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::StructuralScalarFieldStore {
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
        let (byte_offset, byte_size) = crate::structural_inputs::structural_reference_input::store(
            destination.structural_type,
            path,
            *field,
            value.scalar_type,
            &unit.structural_types,
        )
        .ok_or(Error::SourceCustodyMismatch)?;
        LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination: destination.clone(),
            path: path.clone(),
            field: *field,
            value: *value,
            byte_offset,
            byte_size,
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
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::SourceCustodyMismatch)?;
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
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
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
        .ok_or(Error::SourceCustodyMismatch)?;
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::SourceCustodyMismatch)?;
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

pub(super) fn project_structural_byte_sequence_field_store(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
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
        .ok_or(Error::SourceCustodyMismatch)?;
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::SourceCustodyMismatch)?;
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
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::SourceCustodyMismatch)?;
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
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::SourceCustodyMismatch)?;
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
