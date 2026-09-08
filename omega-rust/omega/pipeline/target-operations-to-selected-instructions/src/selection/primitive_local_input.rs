//! Primitive referent geometry and original local producer identity.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind as Instruction,
};
use semantic_vocabulary::{OperationId, PlaceId, ScalarType, StructuralPlaceKind};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape,
};

pub(crate) fn local(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(
    OperationId,
    &StructuralOperationResult,
    ScalarType,
    ValueShape,
)> {
    let signature = source.structural.as_ref()?;
    let mut candidates = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| match &row.kind {
            Instruction::EstablishPrimitiveLocal {
                result,
                value,
                shape,
            } if result.place == place => Some((row, result, value, shape)),
            _ => None,
        });
    let (row, result, value, shape) = candidates.next()?;
    if candidates.next().is_some()
        || row.result.is_some()
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !signature.structural_places.iter().any(|declaration| {
            declaration.id == place
                && declaration.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: row.operation,
                        structural_type: result.structural_type,
                    }
        })
        || !signature.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && declaration.shape == StructuralTypeShape::PrimitiveScalar(value.scalar_type)
        })
        || super::scalar_call_abi::scalar_shape(value.scalar_type) != Some(*shape)
    {
        return None;
    }
    Some((row.operation, result, value.scalar_type, *shape))
}
pub(super) fn accepts(source: &LegalizedScalarFunction) -> bool {
    source.structural.as_ref().is_some_and(|signature| {
        signature.parameters.is_empty()
            && signature.entry_claims.is_empty()
            && source.ranked.is_none()
            && source.parameters.len() == source.call_plan.parameters.len()
            && source
                .parameters
                .iter()
                .zip(&source.call_plan.parameters)
                .all(|(parameter, placement)| parameter.placement == *placement)
            && !signature.structural_places.is_empty()
            && signature
                .structural_places
                .iter()
                .all(|place| local(source, place.id).is_some())
    })
}
pub(super) fn readable(
    source: &LegalizedScalarFunction,
    place: PlaceId,
    scalar: ScalarType,
) -> bool {
    if !matches!(scalar, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed && integer.bits() == 64)
    {
        return false;
    }
    if let Some((_, _, actual, _)) = local(source, place) {
        return actual == scalar;
    }
    source.structural.as_ref().is_some_and(|signature| {
        signature.entry_claims.is_empty()
            && signature.parameters.iter().any(|parameter| {
                parameter.semantic.place == place
                    && matches!(
                        parameter.semantic.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                    && parameter.semantic.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.semantic.qualifications.is_empty()
                    && parameter.semantic.projected_qualifications.is_empty()
                    && signature.structural_types.iter().any(|declaration| {
                        declaration.id == parameter.semantic.structural_type
                            && declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
                    })
            })
    })
}
