//! Constructor members carry memberships from their actual completed sources.
//! A result and an incoming parameter are distinct establishment sites in one
//! place namespace. The destination's declared field type grants no membership;
//! Case/Field/FixedIndex prefixes describe where the transported evidence lives.

use crate::emission::operation_emission::buffer::OperationBuffer;
use terminal_psi::{
    StructuralArgument, StructuralParameterDeclaration, StructuralPathQualification,
    StructuralPathSegment,
};

pub(in crate::unit::attached_unit) fn append<'parameter>(
    argument: &StructuralArgument,
    prefix: &[StructuralPathSegment],
    parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration>,
    operations: &OperationBuffer,
    destination: &mut Vec<StructuralPathQualification>,
) {
    if let Some((_, whole, projected)) = source_memberships(argument.place, parameters, operations)
    {
        let (whole, projected) =
            StructuralPathQualification::select(whole, projected, &argument.path);
        StructuralPathQualification::append_under(&whole, &projected, prefix, destination);
    }
}

pub(in crate::unit::attached_unit) fn source_memberships<'parameter: 'source, 'source>(
    place: semantic_vocabulary::PlaceId,
    parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration>,
    operations: &'source OperationBuffer,
) -> Option<(
    semantic_vocabulary::StructuralTypeId,
    &'source [semantic_vocabulary::StructuralDomainId],
    &'source [StructuralPathQualification],
)> {
    operations
        .iter()
        .filter_map(|operation| operation.result.structural())
        .find(|result| result.place == place)
        .map(|result| {
            (
                result.structural_type,
                result.qualifications.as_slice(),
                result.projected_qualifications.as_slice(),
            )
        })
        .or_else(|| {
            parameters
                .into_iter()
                .find(|parameter| parameter.place == place)
                .map(|parameter| {
                    (
                        parameter.structural_type,
                        parameter.qualifications.as_slice(),
                        parameter.projected_qualifications.as_slice(),
                    )
                })
        })
}
