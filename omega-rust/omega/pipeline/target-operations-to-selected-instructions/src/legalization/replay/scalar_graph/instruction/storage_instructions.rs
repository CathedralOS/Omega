//! Primitive-local, structural-field and byte-sequence instructions
//! replayed against the abstract operation each legalizes.

use super::super::{AbstractOperationPlan, Error, PsiOptimizationUnit};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperation;
use legalized_operations::{LegalizedScalarInstruction, LegalizedScalarInstructionKind};
use semantic_vocabulary::OperationId;

pub(super) fn validate_structural_scalar_field_read(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let ((
        LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, field },
        AbstractOperation::IntegerStructuralField { .. }
        | AbstractOperation::BooleanStructuralField { .. },
    )
    | (
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { source, field },
        AbstractOperation::StructuralByteSequenceFieldLength { .. },
    )) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_structural_scalar_field_read")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (_, _, expected_source, expected_field) = scalar_graph_input::structural_fields::read(
        optimized,
        &node.operation,
        &plan.structural_types,
    )
    .ok_or(invalid.clone())?;
    if source != &expected_source || *field != expected_field {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_write_only_primitive_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
            byte_offset,
            byte_size,
        },
        AbstractOperation::WriteOnlyPrimitiveStore {
            destination: expected,
            path: expected_path,
            value: expected_value,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_write_only_primitive_store")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if destination != expected
        || path != expected_path
        || value != expected_value
        || crate::structural_inputs::structural_reference_input::primitive_store(
            expected,
            expected_path,
            expected_value.scalar_type,
            &unit.structural_types,
        ) != Some((*byte_offset, *byte_size))
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_write_only_indexed_primitive_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::WriteOnlyIndexedPrimitiveStore {
            destination,
            path,
            index,
            value,
            byte_offset,
            byte_size,
            extent,
            obligation,
            accepted_fact,
        },
        AbstractOperation::WriteOnlyIndexedPrimitiveStore {
            destination: expected,
            path: expected_path,
            index: expected_index,
            value: expected_value,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_write_only_indexed_primitive_store")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if destination != expected
        || path != expected_path
        || index != expected_index
        || value != expected_value
        || obligation != expected_obligation
        || crate::structural_inputs::structural_reference_input::indexed_primitive_store(
            expected,
            expected_path,
            expected_value.scalar_type,
            &unit.structural_types,
        ) != Some((*byte_offset, *byte_size, *extent))
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_structural_scalar_field_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            byte_offset,
            byte_size,
        },
        AbstractOperation::StructuralScalarFieldStore {
            destination: expected,
            path: expected_path,
            field: expected_field,
            value: expected_value,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_structural_scalar_field_store")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if destination != expected
        || path != expected_path
        || field != expected_field
        || value != expected_value
        || crate::structural_inputs::structural_reference_input::store(
            expected.structural_type,
            expected_path,
            *expected_field,
            expected_value.scalar_type,
            &unit.structural_types,
        ) != Some((*byte_offset, *byte_size))
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_byte_sequence_subslice(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ByteSequenceSubslice {
            result,
            source,
            start,
            end,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::ByteSequenceSubslice {
            result: expected_result,
            source: expected_source,
            start: expected_start,
            end: expected_end,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_byte_sequence_subslice")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if result != expected_result
        || source != expected_source
        || start != expected_start
        || end != expected_end
        || length != expected_length
        || obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_element_view_subslice(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ElementViewSubslice {
            result,
            source,
            start,
            end,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::ElementViewSubslice {
            result: expected_result,
            source: expected_source,
            start: expected_start,
            end: expected_end,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_element_view_subslice")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if result != expected_result
        || source != expected_source
        || start != expected_start
        || end != expected_end
        || length != expected_length
        || obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_element_view_read(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::ElementViewRead {
            source: expected_source,
            index: expected_index,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_element_view_read")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if source != expected_source
        || index != expected_index
        || length != expected_length
        || obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_structural_byte_sequence_field_byte_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
            destination,
            field,
            index,
            value,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::StructuralByteSequenceFieldByteStore {
            field: expected_field,
            index: expected_index,
            value: expected_value,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_structural_byte_sequence_field_byte_store")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if Some(destination.clone())
        != scalar_graph_input::structural_fields::replacement(
            optimized,
            &node.operation,
            &plan.structural_types,
        )
        || field != expected_field
        || index != expected_index
        || value != expected_value
        || length != expected_length
        || obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_structural_byte_sequence_field_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
            destination,
            field,
            source,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::StructuralByteSequenceFieldStore {
            field: expected_field,
            source: expected_source,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_structural_byte_sequence_field_store")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if Some(destination.clone())
        != scalar_graph_input::structural_fields::replacement(
            optimized,
            &node.operation,
            &plan.structural_types,
        )
        || field != expected_field
        || source != expected_source
        || length != expected_length
        || obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_byte_sequence_write(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::ByteSequenceWrite {
            destination: expected_destination,
            index: expected_index,
            value: expected_value,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_byte_sequence_write")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if destination != expected_destination
        || index != expected_index
        || value != expected_value
        || length != expected_length
        || obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_byte_sequence_read(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ByteSequenceRead {
            source,
            index,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::ByteSequenceRead {
            source: expected_source,
            index: expected_index,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_byte_sequence_read")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if source != expected_source
        || index != expected_index
        || length != expected_length
        || obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(invalid);
    }
    Ok(())
}
