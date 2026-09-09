//! Input-only primitive storage identity; whole-unit custody owns loan and SSA dominance.
use super::*;
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape,
};

pub(super) fn scalar(
    types: &[terminal_psi::StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Option<ScalarType> {
    types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) if declaration.id == identity => {
                Some(scalar)
            }
            _ => None,
        })
}
pub(super) fn producer(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<(
    OperationId,
    &StructuralOperationResult,
    abstract_operations::AbstractResult,
)> {
    let mut matches = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            AbstractOperation::EstablishPrimitiveLocal {
                psi_operation,
                result,
                value,
            } if result.place == place => Some((*psi_operation, result, *value)),
            _ => None,
        });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}
pub(super) fn valid_result(
    function: &PsiOptimizationFunction,
    operation: OperationId,
    result: &StructuralOperationResult,
) -> bool {
    result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && function.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation,
                        structural_type: result.structural_type,
                    }
        })
}
pub(super) fn roster(function: &PsiOptimizationFunction) -> bool {
    function.structural_places.iter().all(|place| {
        producer(function, place.id)
            .is_some_and(|(operation, result, _)| valid_result(function, operation, result))
    }) && function.declared_places
        == function
            .structural_places
            .iter()
            .map(|place| place.id)
            .collect()
}
pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        match &node.operation {
            AbstractOperation::EstablishPrimitiveLocal {
                psi_operation,
                result,
                value,
            } => {
                if !valid_result(function, *psi_operation, result)
                    || scalar(types, result.structural_type) != Some(value.scalar_type)
                    || scalar_shape(value.scalar_type).is_none()
                    || value_type(function, value.value) != Some(value.scalar_type)
                {
                    return Err(invalid);
                }
            }
            AbstractOperation::PrimitiveLocalStore {
                destination, value, ..
            } => {
                let (operation, result, _) =
                    producer(function, *destination).ok_or(invalid.clone())?;
                if !valid_result(function, operation, result)
                    || scalar(types, result.structural_type) != Some(value.scalar_type)
                    || scalar_shape(value.scalar_type).is_none()
                    || value_type(function, value.value) != Some(value.scalar_type)
                {
                    return Err(invalid);
                }
            }
            AbstractOperation::PrimitiveScalarRead { source, result, .. } => {
                let identity = if let Some((operation, local, _)) = producer(function, *source) {
                    if !valid_result(function, operation, local) {
                        return Err(invalid);
                    }
                    local.structural_type
                } else {
                    let parameter = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == *source)
                        .ok_or(invalid.clone())?;
                    if !matches!(
                        parameter.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                        || !function.entry_claims.is_empty()
                    {
                        return Err(invalid);
                    }
                    parameter.structural_type
                };
                if scalar(types, identity) != Some(result.scalar_type)
                    || scalar_shape(result.scalar_type).is_none()
                {
                    return Err(invalid);
                }
            }
            _ => {}
        }
    }
    Ok(())
}
