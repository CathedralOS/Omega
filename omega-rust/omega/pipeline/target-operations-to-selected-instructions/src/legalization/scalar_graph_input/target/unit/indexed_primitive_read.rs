//! Receiving indexed primitive reads retain the exact index source beside
//! the declared referent. The runtime `u64` index must resolve to an exact
//! dominating source the same way a stored scalar does, and the read's scalar
//! result registers as a home requirement for downstream consumers.
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
    sources: &mut Vec<(ValueId, Source)>,
    _optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetUnitOperation::IndexedPrimitiveRead {
        psi_operation,
        result,
        source,
        path,
        index,
        source_type,
        source_placement,
        obligation,
    } = target
    else {
        return Err(invalid);
    };
    let AbstractOperation::IndexedPrimitiveRead {
        psi_operation: expected_operation,
        result: expected_result,
        source: expected_source,
        path: expected_path,
        index: expected_index,
        obligation: expected_obligation,
    } = abstracted
    else {
        return Err(invalid);
    };
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.place == expected_source.place)
        .ok_or(invalid.clone())?;
    let declared_type = unit
        .structural_types
        .iter()
        .find(|declaration| declaration.id == expected_source.structural_type)
        .ok_or(invalid.clone())?;
    let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
    if psi_operation != expected_operation
        || result != expected_result
        || source != expected_source
        || path != expected_path
        || source_type != declared_type
        || source_placement != &parameter.placement
        || obligation != expected_obligation
        || crate::structural_inputs::structural_reference_input::indexed_primitive_read(
            expected_source,
            expected_path,
            expected_result.scalar_type,
            &unit.structural_types,
        )
        .is_none()
        || index.source_value() != expected_index.value
        || index.scalar_type() != expected_index.scalar_type
        || index.scalar_type() != ScalarType::Integer(unsigned_64)
        || !sources
            .iter()
            .any(|(identity, expected)| *identity == expected_index.value && index == expected)
    {
        return Err(invalid);
    }
    sources.push((
        result.value,
        Source::Home(target_operations::TargetUnitScalarHomeRequirement {
            defining_operation: *psi_operation,
            source_value: result.value,
            scalar_type: result.scalar_type,
            shape: crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                .ok_or(invalid.clone())?,
        }),
    ));
    Ok(())
}
