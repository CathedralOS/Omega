//! Receiving indexed primitive stores retain the exact index and source
//! beside the declared referent. The runtime `u64` index must resolve to an
//! exact dominating source the same way the stored scalar does.
use crate::LegalizationError;
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
use target_operations::{
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource as Source,
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
    let TargetUnitOperation::WriteOnlyIndexedPrimitiveStore {
        psi_operation,
        destination,
        path,
        index,
        destination_type,
        destination_placement,
        source,
        obligation,
    } = target
    else {
        return Err(invalid);
    };
    let AbstractOperation::WriteOnlyIndexedPrimitiveStore {
        psi_operation: expected_operation,
        destination: expected_destination,
        path: expected_path,
        index: expected_index,
        value,
        obligation: expected_obligation,
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
    let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
    if psi_operation != expected_operation
        || destination != expected_destination
        || path != expected_path
        || destination_type != declared_type
        || destination_placement != &parameter.placement
        || obligation != expected_obligation
        || crate::structural_inputs::structural_reference_input::indexed_primitive_store(
            expected_destination,
            expected_path,
            value.scalar_type,
            &unit.structural_types,
        )
        .is_none()
        || index.source_value() != expected_index.value
        || index.scalar_type() != expected_index.scalar_type
        || index.scalar_type() != ScalarType::Integer(unsigned_64)
        || source.source_value() != value.value
        || source.scalar_type() != value.scalar_type
        || !sources
            .iter()
            .any(|(identity, expected)| *identity == expected_index.value && index == expected)
        || !(sources
            .iter()
            .any(|(identity, expected)| *identity == value.value && source == expected)
            || preceding_ieee_literal(source, *expected_operation, optimized))
    {
        return Err(invalid);
    }
    Ok(())
}

/// A literal store requires its exact IEEE definition earlier in the same
/// block. This indexed-store check retains that same-block ordering.
fn preceding_ieee_literal(
    source: &Source,
    store: semantic_vocabulary::OperationId,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> bool {
    let Source::IeeeFloatImmediate {
        defining_operation,
        source_value,
        value,
    } = source
    else {
        return false;
    };
    optimized.blocks.iter().any(|block| {
        let Some(store_position) = block.nodes.iter().position(|node| matches!(node.operation,
            AbstractOperation::WriteOnlyIndexedPrimitiveStore { psi_operation, .. } if psi_operation == store)) else {
            return false;
        };
        block.nodes[..store_position].iter().any(|node| matches!(node.operation,
            AbstractOperation::IeeeFloatConstant { psi_operation, result, value: literal }
                if psi_operation == *defining_operation && result == *source_value && literal == *value))
    })
}
