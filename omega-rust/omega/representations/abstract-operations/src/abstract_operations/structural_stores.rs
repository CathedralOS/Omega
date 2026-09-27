//! The semantic storage authority retained by a scalar field replacement.
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
};

/// A borrowed parameter and an owned operation result have different custody.
/// An owned home retains its original producer; it never acquires a parameter
/// position or a borrowed ABI identity merely because a field is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralScalarStoreDestination {
    Parameter(StructuralParameterDeclaration),
    OwnedResult {
        producer: OperationId,
        result: StructuralOperationResult,
    },
}

impl StructuralScalarStoreDestination {
    pub fn place(&self) -> PlaceId {
        match self {
            Self::Parameter(parameter) => parameter.place,
            Self::OwnedResult { result, .. } => result.place,
        }
    }

    pub fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::Parameter(parameter) => parameter.structural_type,
            Self::OwnedResult { result, .. } => result.structural_type,
        }
    }

    pub fn multiplicity(&self) -> StructuralMultiplicity {
        match self {
            Self::Parameter(parameter) => parameter.multiplicity,
            Self::OwnedResult { result, .. } => result.multiplicity,
        }
    }

    pub fn parameter(&self) -> Option<&StructuralParameterDeclaration> {
        match self {
            Self::Parameter(parameter) => Some(parameter),
            Self::OwnedResult { .. } => None,
        }
    }
}

impl From<StructuralParameterDeclaration> for StructuralScalarStoreDestination {
    fn from(parameter: StructuralParameterDeclaration) -> Self {
        Self::Parameter(parameter)
    }
}

/// The root a borrowed-window restoration pair operates under. A
/// mutable-borrowed machine parameter carries its complete declared row; an
/// owned local home retains the producing operation and its result row — the
/// same admission the Terminal verifier's `borrowed_window_root` and the
/// codec's `borrowed_field_store_root_type` apply, so a completed local home
/// can carry a restoration window beside a borrowed parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralWindowRoot {
    /// Mutable-borrowed signature parameter — retains the complete declared
    /// row: borrow authority, multiplicity, and nominal type.
    BorrowedParameter(StructuralParameterDeclaration),
    /// Owned local home completed by a producer operation (record/array
    /// establishment, structural call result, leaf copy) under
    /// `Unrestricted`/`Affine` custody with no qualifications or claims.
    OwnedResult {
        producer: OperationId,
        result: StructuralOperationResult,
    },
}

impl StructuralWindowRoot {
    pub fn place(&self) -> PlaceId {
        match self {
            Self::BorrowedParameter(parameter) => parameter.place,
            Self::OwnedResult { result, .. } => result.place,
        }
    }

    pub fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::BorrowedParameter(parameter) => parameter.structural_type,
            Self::OwnedResult { result, .. } => result.structural_type,
        }
    }

    pub fn parameter(&self) -> Option<&StructuralParameterDeclaration> {
        match self {
            Self::BorrowedParameter(parameter) => Some(parameter),
            Self::OwnedResult { .. } => None,
        }
    }
}
