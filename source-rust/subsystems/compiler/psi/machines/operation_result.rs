use crate::{
    StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment, ValueDeclaration,
};
use semantic_vocabulary::{ClaimId, PlaceId, StructuralDomainId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationResult {
    Unit,
    Scalar(ValueDeclaration),
    Structural(StructuralOperationResult),
}

impl OperationResult {
    pub fn as_structural(&self) -> Option<&StructuralOperationResult> {
        let OperationResult::Structural(structural) = self else {
            return None;
        };
        Some(structural)
    }

    pub fn as_scalar(&self) -> Option<ValueDeclaration> {
        let OperationResult::Scalar(scalar) = self else {
            return None;
        };
        Some(*scalar)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralOperationResult {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub qualifications: Vec<StructuralDomainId>,
    pub projected_qualifications: Vec<StructuralPathQualification>,
    pub additional_input_qualifications: Vec<StructuralPathQualification>,
    pub qualification_establishments: Vec<crate::ResultQualificationEstablishment>,
    pub claims: Vec<StructuralResultClaimBinding>,
}

impl StructuralOperationResult {
    pub fn is_unrestricted_plain(&self) -> bool {
        self.multiplicity == StructuralMultiplicity::Unrestricted
            && self.qualifications.is_empty()
            && self.projected_qualifications.is_empty()
            && self.claims.is_empty()
    }

    pub fn is_affine_plain(&self) -> bool {
        self.multiplicity == StructuralMultiplicity::Affine
            && self.qualifications.is_empty()
            && self.projected_qualifications.is_empty()
            && self.claims.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultClaimBinding {
    pub claim: ClaimId,
    pub path: Vec<StructuralPathSegment>,
}

impl OperationResult {
    pub const fn scalar(&self) -> Option<ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(*value),
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(value),
        }
    }

    record_storage::enum_variant_accessor!(structural, Structural; StructuralOperationResult; Self::Unit | Self::Scalar(_));

    pub const fn expect_scalar(&self) -> ValueDeclaration {
        match self {
            Self::Scalar(value) => *value,
            Self::Unit | Self::Structural(_) => panic!("operation has no scalar result"),
        }
    }
}
