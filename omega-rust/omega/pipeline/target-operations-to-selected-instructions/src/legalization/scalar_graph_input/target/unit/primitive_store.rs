//! Receiving primitive stores retains the exact source and declared referent.
use crate::LegalizationError;
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::ValueId;
use target_operations::{
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource as Source,
    TargetUnitWriteOnlyPrimitiveStoreSource as PrimitiveSource,
};

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    parameters: &[TargetStructuralParameter],
    sources: &[(ValueId, Source)],
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetUnitOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        destination_type,
        destination_placement,
        source,
    } = target
    else {
        return Err(invalid);
    };
    let AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation: expected_operation,
        destination: expected_destination,
        value,
    } = abstracted
    else {
        return Err(invalid);
    };
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.place == expected_destination.place)
        .ok_or(invalid.clone())?;
    let declared_type = unit
        .structural_types
        .iter()
        .find(|declaration| declaration.id == expected_destination.structural_type)
        .ok_or(invalid.clone())?;
    if psi_operation != expected_operation
        || destination != expected_destination
        || destination_type != declared_type
        || destination_placement != &parameter.placement
        || crate::structural_reference_input::primitive_store(
            expected_destination,
            value.scalar_type,
            &unit.structural_types,
        )
        .is_none()
        || source.source_value() != value.value
        || source.scalar_type() != value.scalar_type
        || !(sources.iter().any(|(identity, expected)| {
            *identity == value.value && source_is_exact(source, expected)
        }) || preceding_ieee_literal(source, *expected_operation, optimized))
    {
        return Err(invalid);
    }
    Ok(())
}

/// A literal store requires its exact definition earlier in the same block.
/// This primitive-specific check also retains the store's same-block ordering.
fn preceding_ieee_literal(
    source: &PrimitiveSource,
    store: semantic_vocabulary::OperationId,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> bool {
    let PrimitiveSource::IeeeFloatImmediate {
        defining_operation,
        source_value,
        value,
    } = source
    else {
        return false;
    };
    optimized.blocks.iter().any(|block| {
        let Some(store_position) = block.nodes.iter().position(|node| matches!(node.operation,
            AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. } if psi_operation == store)) else {
            return false;
        };
        block.nodes[..store_position].iter().any(|node| matches!(node.operation,
            AbstractOperation::IeeeFloatConstant { psi_operation, result, value: literal }
                if psi_operation == *defining_operation && result == *source_value && literal == *value))
    })
}

// Primitive-store literals and call arguments have separate source vocabularies.
// IEEE primitive literals retain the preceding-definition check above.
fn source_is_exact(source: &PrimitiveSource, expected: &Source) -> bool {
    match (source, expected) {
        (
            PrimitiveSource::Parameter {
                parameter_index,
                source_value,
                scalar_type,
            },
            Source::Parameter {
                parameter_index: expected_index,
                source_value: expected_value,
                scalar_type: expected_type,
            },
        ) => {
            parameter_index == expected_index
                && source_value == expected_value
                && scalar_type == expected_type
        }
        (
            PrimitiveSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            },
            Source::IntegerImmediate {
                defining_operation: expected_operation,
                source_value: expected_value,
                scalar_type: expected_type,
                value: expected_literal,
            },
        ) => {
            defining_operation == expected_operation
                && source_value == expected_value
                && scalar_type == expected_type
                && value == expected_literal
        }
        (
            PrimitiveSource::BooleanImmediate {
                defining_operation,
                source_value,
                value,
            },
            Source::BooleanImmediate {
                defining_operation: expected_operation,
                source_value: expected_value,
                value: expected_literal,
            },
        ) => {
            defining_operation == expected_operation
                && source_value == expected_value
                && value == expected_literal
        }
        (PrimitiveSource::Home(home), Source::Home(expected)) => home == expected,
        _ => false,
    }
}
