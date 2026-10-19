//! Structural result validation for an already admitted single-attempt event.
//! This query does not select an atomic operation from a sum's shape. Source
//! admission establishes the sealed family; downstream consumers separately
//! rejoin the event, exact resident scalar and its whole plain result.
use crate::{
    StructuralCaseDeclaration, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};
use semantic_vocabulary::ScalarType;

pub fn atomic_once_outcome_cases(
    declaration: &StructuralTypeDeclaration,
    resident: ScalarType,
) -> Option<&[StructuralCaseDeclaration; 3]> {
    if declaration.quotient_carrier.is_some()
        || declaration.owns_linear_claim
        || declaration.scalar_restriction.is_some()
    {
        return None;
    }
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let cases: &[StructuralCaseDeclaration; 3] = cases.as_slice().try_into().ok()?;
    if !cases[1].fields.is_empty() {
        return None;
    }
    for case in [&cases[0], &cases[2]] {
        let [field] = case.fields.as_slice() else {
            return None;
        };
        if field.relevance.is_erased()
            || field.field_type != StructuralFieldType::Scalar(resident)
            || field.storage.is_some()
        {
            return None;
        }
    }
    Some(cases)
}
