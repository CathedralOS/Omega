//! Canonical structural shapes for atomic outcomes.

use crate::{
    StructuralCaseDeclaration, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};
use language_core::atomic::AtomicCompareExchangeObservationAxis;
use semantic_vocabulary::ScalarType;

pub fn atomic_once_outcome_cases(
    declaration: &StructuralTypeDeclaration,
    resident: ScalarType,
    custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
) -> Option<&[StructuralCaseDeclaration; 3]> {
    let multiplicity = match custody.observation {
        AtomicCompareExchangeObservationAxis::Observing => {
            crate::StructuralMultiplicity::Unrestricted
        }
        AtomicCompareExchangeObservationAxis::NonObserving => crate::StructuralMultiplicity::Affine,
    };
    // The event establishes a plain outcome. It cannot prove a receiving
    // declaration's additional invariant or acquire a cleanup obligation.
    if declaration.multiplicity != multiplicity
        || !declaration.requires.is_empty()
        || declaration.nominal_cleanup.is_some()
        || !custody.is_canonical()
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

pub fn atomic_once_result_is_plain(
    result: &crate::StructuralOperationResult,
    custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
) -> bool {
    custody.is_canonical()
        && result.additional_input_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && match custody.observation {
            AtomicCompareExchangeObservationAxis::Observing => result.is_unrestricted_plain(),
            AtomicCompareExchangeObservationAxis::NonObserving => result.is_affine_plain(),
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StructuralFieldDeclaration, StructuralMultiplicity};
    use language_core::BindingRelevance;

    #[test]
    fn outcome_cannot_establish_another_multiplicity_or_declaration_predicate() {
        use language_core::atomic::{
            AtomicCompareExchangeObservationAxis::Observing, AtomicCompareExchangeOnceResultCustody,
        };
        use semantic_vocabulary::{
            Proposition, StructuralCaseId, StructuralFieldId, StructuralTypeId,
        };
        let custody = AtomicCompareExchangeOnceResultCustody::for_observation(Observing);
        let field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: Default::default(),
            declaration_identity: Default::default(),
            inline_carrier: None,
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            storage: None,
        };
        let cases = [vec![field.clone()], vec![], vec![field]]
            .into_iter()
            .enumerate()
            .map(|(index, fields)| StructuralCaseDeclaration {
                id: StructuralCaseId::new(index as u64 + 1).unwrap(),
                identity: Default::default(),
                declaration_identity: Default::default(),
                fields,
            })
            .collect();
        let mut declaration = StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: Default::default(),
            declaration_identity: Default::default(),
            scalar_restriction: None,
            quotient_carrier: None,
            owns_linear_claim: false,
            multiplicity: StructuralMultiplicity::Unrestricted,
            nominal_cleanup: None,
            descriptor_cleanup: true,
            no_code_affine_cleanup: true,
            shape: StructuralTypeShape::Sum { cases },
            requires: Vec::new(),
        };
        let valid = |declaration: &StructuralTypeDeclaration| {
            atomic_once_outcome_cases(declaration, ScalarType::Boolean, custody).is_some()
        };
        assert!(valid(&declaration));
        declaration.multiplicity = StructuralMultiplicity::Linear;
        assert!(!valid(&declaration));
        declaration.multiplicity = StructuralMultiplicity::Unrestricted;
        declaration.requires.push(Proposition::Falsehood);
        assert!(!valid(&declaration));
    }
}
