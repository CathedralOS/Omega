//! Input-only primitive storage identity; whole-unit custody owns loan and SSA dominance.
use super::{AbstractOperation, PsiOptimizationFunction, ScalarType, scalar_shape};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::value_type;
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
        && valid_owned_result(function, operation, result)
}

fn valid_owned_result(
    function: &PsiOptimizationFunction,
    operation: OperationId,
    result: &StructuralOperationResult,
) -> bool {
    matches!(
        result.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) && result.qualifications.is_empty()
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
/// The parameter a primitive read's root names, independent of its native
/// carrier. Access, multiplicity and path rules stay with each
/// caller; this only finds the declaration naming `place`.
pub(in crate::legalization) fn read_parameter(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralParameterDeclaration> {
    function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
}

/// The declared storage type beneath a primitive read: an owned
/// establishment's result, else the parameter `read_parameter` finds.
pub(in crate::legalization) fn read_root_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    super::structural_case::source_result(function, place)
        .ok()
        .map(|(_, result)| result.structural_type)
        .or_else(|| {
            read_parameter(function, place).map(|parameter| parameter.structural_type)
        })
}

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
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
                    return Err(LegalizationError::custody());
                }
            }
            AbstractOperation::PrimitiveLocalStore {
                destination, value, ..
            } => {
                let (operation, result, _) =
                    producer(function, *destination).ok_or(LegalizationError::custody())?;
                if !valid_result(function, operation, result)
                    || scalar(types, result.structural_type) != Some(value.scalar_type)
                    || scalar_shape(value.scalar_type).is_none()
                    || value_type(function, value.value) != Some(value.scalar_type)
                {
                    return Err(LegalizationError::custody());
                }
            }
            AbstractOperation::PrimitiveScalarRead {
                source,
                path,
                result,
                ..
            } => {
                let identity = if let Some((operation, local, _)) = producer(function, *source) {
                    if !path.is_empty() || !valid_result(function, operation, local) {
                        return Err(LegalizationError::custody());
                    }
                    local.structural_type
                } else if let Ok((operation, local)) =
                    super::structural_case::source_result(function, *source)
                {
                    // An aggregate establishment owns the original referent.
                    // Its declaration, restrictions and complete projection
                    // remain independently checked just like a parameter's.
                    if path.is_empty() || !valid_owned_result(function, operation, local) {
                        return Err(LegalizationError::custody());
                    }
                    local.structural_type
                } else {
                    let parameter = read_parameter(function, *source)
                        .ok_or(LegalizationError::custody())?;
                    if !(matches!(
                        parameter.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    ) || (parameter.access == StructuralAccess::Owned && !path.is_empty()))
                        || parameter.multiplicity == StructuralMultiplicity::Linear
                        || (path.is_empty()
                            && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                        || !function.entry_claims.is_empty()
                    {
                        return Err(LegalizationError::custody());
                    }
                    parameter.structural_type
                };
                if crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                    identity,
                    path,
                    result.scalar_type,
                    types,
                )
                .is_none()
                    || scalar_shape(result.scalar_type).is_none()
                {
                    return Err(LegalizationError::custody());
                }
            }
            _ => {}
        }
    }
    Ok(())
}
