//! Constructor members carry memberships from their actual completed sources.
//! A result and an incoming parameter are distinct establishment sites in one
//! place namespace. The destination's declared field type grants no membership;
//! Case/Field/FixedIndex prefixes describe where the transported evidence lives.

use crate::emission::operation_emission::buffer::OperationBuffer;
use terminal_psi::{
    StructuralArgument, StructuralParameterDeclaration, StructuralPathQualification,
    StructuralPathSegment,
};

pub(super) fn append<'parameter>(
    argument: &StructuralArgument,
    prefix: &[StructuralPathSegment],
    parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration>,
    operations: &OperationBuffer,
    destination: &mut Vec<StructuralPathQualification>,
) {
    let source = operations
        .iter()
        .filter_map(|operation| operation.result.structural())
        .find(|result| result.place == argument.place)
        .map(|result| (&result.qualifications, &result.projected_qualifications))
        .or_else(|| {
            parameters
                .into_iter()
                .find(|parameter| parameter.place == argument.place)
                .map(|parameter| {
                    (
                        &parameter.qualifications,
                        &parameter.projected_qualifications,
                    )
                })
        });
    if let Some((whole, projected)) = source {
        let (whole, projected) =
            StructuralPathQualification::select(whole, projected, &argument.path);
        StructuralPathQualification::append_under(&whole, &projected, prefix, destination);
    }
}
