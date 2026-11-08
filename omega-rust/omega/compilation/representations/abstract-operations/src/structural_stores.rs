// We retain the semantic storage authority of scalar field stores, primitive
// stores, restoration windows, and unrestricted leaf copy paths. A borrowed
// parameter and an owned operation or block-entry result have distinct custody;
// keeping their exact declarations prevents an owned home from acquiring a
// borrowed ABI identity when written, and keeps case-qualified copy paths from
// collapsing into flat field names.

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, OperationId, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralCopyPath {
    Ordinary(Vec<StructuralPathSegment>),
    Case(Vec<CanonicalStructuralPathSegment>),
}

impl From<Vec<StructuralPathSegment>> for StructuralCopyPath {
    fn from(path: Vec<StructuralPathSegment>) -> Self {
        Self::Ordinary(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralScalarStoreDestination {
    Parameter(StructuralParameterDeclaration), // Entry ABI slot (owned or borrowed) or non-entry address join carrying a loan.
    OwnedResult {
        producer: OperationId,
        result: StructuralOperationResult,
    },
    OwnedBlockParameter {
        block: BlockId, // Block entry producer identity for whole owned values arriving on a block edge.
        parameter: StructuralParameterDeclaration,
    },
}

macro_rules! store_destination_field {
    ($name:ident, $field:ident -> $ty:ty) => {
        pub fn $name(&self) -> $ty {
            match self {
                Self::Parameter(parameter) | Self::OwnedBlockParameter { parameter, .. } => {
                    parameter.$field
                }
                Self::OwnedResult { result, .. } => result.$field,
            }
        }
    };
}

impl StructuralScalarStoreDestination {
    store_destination_field!(place, place -> PlaceId);
    store_destination_field!(structural_type, structural_type -> StructuralTypeId);
    store_destination_field!(multiplicity, multiplicity -> StructuralMultiplicity);

    pub fn qualifications(&self) -> &[semantic_vocabulary::StructuralDomainId] {
        match self {
            Self::Parameter(value)
            | Self::OwnedBlockParameter {
                parameter: value, ..
            } => &value.qualifications,
            Self::OwnedResult { result, .. } => &result.qualifications,
        }
    }

    pub fn projected_qualifications(&self) -> &[terminal_psi::StructuralPathQualification] {
        match self {
            Self::Parameter(value)
            | Self::OwnedBlockParameter {
                parameter: value, ..
            } => &value.projected_qualifications,
            Self::OwnedResult { result, .. } => &result.projected_qualifications,
        }
    }

    pub fn additional_input_qualifications(&self) -> &[terminal_psi::StructuralPathQualification] {
        match self {
            Self::Parameter(value)
            | Self::OwnedBlockParameter {
                parameter: value, ..
            } => &value.additional_input_qualifications,
            Self::OwnedResult { result, .. } => &result.additional_input_qualifications,
        }
    }

    pub fn parameter(&self) -> Option<&StructuralParameterDeclaration> {
        match self {
            Self::Parameter(parameter) if parameter.access != StructuralAccess::Owned => {
                Some(parameter)
            }
            Self::Parameter(_) => None,
            Self::OwnedResult { .. } | Self::OwnedBlockParameter { .. } => None,
        }
    }
}

impl From<StructuralParameterDeclaration> for StructuralScalarStoreDestination {
    fn from(parameter: StructuralParameterDeclaration) -> Self {
        Self::Parameter(parameter)
    }
}

pub type StructuralWindowRoot = StructuralScalarStoreDestination;
