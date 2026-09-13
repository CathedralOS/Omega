//! Bind source field identities to the exact emitted receiver declaration.

use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct StructuralScalarFieldBinding {
    pub source_position: u32,
    pub identity: String,
    pub source: PlaceId,
    pub field: StructuralFieldId,
    pub scalar_type: ScalarType,
}

impl StructuralScalarFieldBinding {
    pub(crate) fn collect(
        parameters: &[(u32, StructuralParameterDeclaration)],
        types: &[StructuralTypeDeclaration],
    ) -> Vec<Self> {
        let mut bindings = Vec::new();
        for (position, parameter) in parameters {
            if !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
            ) {
                continue;
            }
            let Some(declaration) = types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
            else {
                continue;
            };
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                continue;
            };
            for field in fields {
                let scalar_type = match field.field_type {
                    StructuralFieldType::Scalar(scalar_type) => scalar_type,
                    StructuralFieldType::BoundedInteger(integer) => {
                        ScalarType::Integer(integer.integer_type())
                    }
                    _ => continue,
                };
                if field.relevance.is_erased() {
                    continue;
                }
                bindings.push(StructuralScalarFieldBinding {
                    source_position: *position,
                    identity: field.identity.clone(),
                    source: parameter.place,
                    field: field.id,
                    scalar_type,
                });
            }
        }
        bindings
    }
}

pub(crate) fn resolve(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<(PlaceId, StructuralFieldId), LoweringError> {
    let [checked_trees::CheckedStructuralPredicatePathSegment::Field(identity)] = path else {
        return unsupported("runtime field observation requires one direct relevant scalar field");
    };
    let mut matching = fields.iter().filter(|field| {
        field.source_position == position
            && field.identity == *identity
            && field.scalar_type == scalar_type
    });
    let field = matching.next().ok_or(LoweringError::Unsupported(
        "runtime field observation has no exact readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("runtime field observation has ambiguous bindings");
    }
    Ok((field.source, field.field))
}
