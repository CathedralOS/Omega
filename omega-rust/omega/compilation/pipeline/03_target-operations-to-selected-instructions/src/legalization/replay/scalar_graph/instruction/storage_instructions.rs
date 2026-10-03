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
    unit: &PsiOptimizationUnit,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    // A record read and a byte-length observation carry no case; a
    // case-qualified read must name exactly the case its abstract carrier's
    // final `Case` step names, so neither a dropped nor a substituted case
    // (which would move the offset to another overlay) replays.
    let (source, case, field) = match (&actual.kind, &node.operation) {
        (
            LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                source,
                case,
                field,
            },
            AbstractOperation::StructuralScalarFieldRead { .. },
        ) => (source, *case, field),
        (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                source, field, ..
            },
            AbstractOperation::StructuralByteSequenceFieldLength { .. },
        ) => (source, None, field),
        _ => unreachable!("dispatched validate_structural_scalar_field_read"),
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (_, _, expected_source, expected_case, expected_field) =
        scalar_graph_input::structural_fields::read(
            optimized,
            &node.operation,
            &plan.structural_types,
            custody,
        )
        .ok_or(invalid.clone())?;
    if let LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { indices, .. } =
        &actual.kind
        && *indices
            != scalar_graph_input::structural_fields::byte_index_operands(
                optimized,
                unit,
                actual.operation,
                source,
                *field,
            )?
    {
        return Err(invalid);
    }
    if source != &expected_source || case != expected_case || *field != expected_field {
        return Err(invalid);
    }
    Ok(())
}

/// A primitive read's runtime elements rejoin their selectors and the
/// verifier's certificates; a static read carries none.
pub(super) fn validate_primitive_scalar_read(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::PrimitiveScalarRead {
            source,
            path,
            indices,
        },
        AbstractOperation::PrimitiveScalarRead {
            source: expected,
            path: expected_path,
            result,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_primitive_scalar_read")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let expected_indices = if terminal_psi::is_static_structural_path(expected_path) {
        Vec::new()
    } else {
        let root_type = scalar_graph_input::primitive_locals::read_root_type(optimized, *expected)
            .ok_or(invalid.clone())?;
        let (_, _, elements) =
            crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                root_type,
                expected_path,
                result.scalar_type,
                &unit.structural_types,
            )
            .ok_or(invalid.clone())?;
        crate::legalization::runtime_indices::operands(optimized, unit, operation, &elements)?
    };
    if source != expected || path != expected_path || *indices != expected_indices {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_write_only_primitive_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
            byte_offset,
            byte_size,
            indices,
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
    let (expected_offset, expected_size, elements) =
        crate::structural_inputs::structural_reference_input::primitive_store(
            expected,
            expected_path,
            expected_value.scalar_type,
            &unit.structural_types,
        )
        .ok_or(invalid.clone())?;
    if destination != expected
        || path != expected_path
        || value != expected_value
        || (*byte_offset, *byte_size) != (expected_offset, expected_size)
        || *indices
            != crate::legalization::runtime_indices::operands(
                optimized, unit, operation, &elements,
            )?
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_structural_scalar_field_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            byte_offset,
            byte_size,
            indices,
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
    let (expected, expected_path) =
        scalar_graph_input::structural_fields::scalar_store_destination(
            optimized,
            expected,
            expected_path,
            &unit.structural_types,
            custody,
        )
        .ok_or(invalid.clone())?;
    let (expected_offset, expected_size, elements) =
        crate::structural_inputs::structural_reference_input::store(
            expected.structural_type(),
            &expected_path,
            *expected_field,
            expected_value.scalar_type,
            &unit.structural_types,
        )
        .ok_or(invalid.clone())?;
    if destination != &expected
        || path != &expected_path
        || field != expected_field
        || value != expected_value
        || (*byte_offset, *byte_size) != (expected_offset, expected_size)
        || *indices
            != crate::legalization::runtime_indices::operands(
                optimized, unit, operation, &elements,
            )?
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
            path,
        },
        AbstractOperation::ElementViewRead {
            source: expected_source,
            index: expected_index,
            length: expected_length,
            obligation: expected_obligation,
            path: expected_path,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_element_view_read")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if path != expected_path
        || source != expected_source
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
    custody: &scalar_graph_input::reference_custody::Custody,
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
            custody,
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

pub(super) fn validate_structural_byte_sequence_field_read(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
            indices,
            source,
            field,
            index,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::StructuralByteSequenceFieldRead {
            field: expected_field,
            index: expected_index,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_structural_byte_sequence_field_read")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if *indices
        != scalar_graph_input::structural_fields::byte_index_operands(
            optimized, unit, operation, source, *field,
        )?
    {
        return Err(invalid);
    }
    if Some(source.clone())
        != scalar_graph_input::structural_fields::byte_read(
            optimized,
            &node.operation,
            &plan.structural_types,
        )
        || field != expected_field
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

pub(super) fn validate_structural_byte_sequence_field_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    custody: &scalar_graph_input::reference_custody::Custody,
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
            custody,
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

pub(super) fn validate_element_view_write(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ElementViewWrite {
            path,
            destination,
            index,
            value,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::ElementViewWrite {
            path: expected_path,
            destination: expected_destination,
            index: expected_index,
            value: expected_value,
            length: expected_length,
            obligation: expected_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_element_view_write")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if path != expected_path
        || destination != expected_destination
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

pub(super) fn validate_byte_sequence_concatenation(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
            result,
            left,
            right,
            capacity_obligation,
            accepted_fact,
        },
        AbstractOperation::EstablishByteSequenceConcatenation {
            psi_operation,
            result: expected_result,
            left: expected_left,
            right: expected_right,
            capacity_obligation: expected_obligation,
        },
    ) = (&actual.kind, &node.operation)
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    if actual.operation != *psi_operation
        || result != expected_result
        || left != expected_left
        || right != expected_right
        || capacity_obligation != expected_obligation
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == *psi_operation
                && fact.obligation == *capacity_obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

pub(super) fn validate_byte_sequence_store(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ByteSequenceStore {
            destination,
            source,
            length,
            obligation,
            accepted_fact,
        },
        AbstractOperation::ByteSequenceStore {
            psi_operation,
            destination: expected_destination,
            source: expected_source,
            length: expected_length,
            obligation: expected_obligation,
        },
    ) = (&actual.kind, &node.operation)
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    if operation != *psi_operation
        || destination != expected_destination
        || source != expected_source
        || length != expected_length
        || obligation != expected_obligation
        || scalar_graph_input::structural_fields::replacement(
            optimized,
            &node.operation,
            &plan.structural_types,
            custody,
        )
        .is_none()
        || !unit.accepted_obligation_facts.iter().any(|fact| {
            fact.machine == optimized.machine
                && fact.operation == operation
                && fact.obligation == *obligation
                && fact.identity == *accepted_fact
        })
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}
