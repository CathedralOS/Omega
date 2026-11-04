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
    custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
) -> Option<&[StructuralCaseDeclaration; 3]> {
    if !custody.is_canonical()
        || declaration.quotient_carrier.is_some()
        || declaration.owns_linear_claim
        || declaration.scalar_restriction.is_some()
    {
        return None;
    }
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let cases: &[StructuralCaseDeclaration; 3] = cases.as_slice().try_into().ok()?;
    for (case, schema) in cases.iter().zip(custody.outcome_identity.cases()) {
        match schema.payload {
            None if case.fields.is_empty() => {}
            Some(_) => {
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
            _ => return None,
        }
    }
    Some(cases)
}

/// The canonical result owns exactly the value its admitted family produces.
/// No result qualification or claim may be invented from an outcome shape.
pub fn atomic_once_result_is_plain(
    result: &crate::StructuralOperationResult,
    custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
) -> bool {
    use language_core::atomic::AtomicCompareExchangeObservationAxis;
    custody.is_canonical()
        && result.qualification_establishments.is_empty()
        && match custody.observation {
            AtomicCompareExchangeObservationAxis::Observing => result.is_unrestricted_plain(),
            AtomicCompareExchangeObservationAxis::NonObserving => result.is_affine_plain(),
        }
}
