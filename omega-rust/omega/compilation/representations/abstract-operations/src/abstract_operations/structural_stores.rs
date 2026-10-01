//! The semantic storage authority retained by scalar field and primitive stores.
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
};

/// A borrowed parameter and an owned operation result have different custody.
/// An owned home retains its original producer; it never acquires a parameter
/// position or a borrowed ABI identity merely because a field is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralScalarStoreDestination {
    /// A mutable or write-only loan: an entry parameter's ABI slot, or a
    /// non-entry block's address join carrying its referent's address.
    Parameter(StructuralParameterDeclaration),
    OwnedResult {
        producer: OperationId,
        result: StructuralOperationResult,
    },
    /// A whole owned value that arrived on a block edge and lives in that
    /// block's own home (Terminal `structural_access.md`: an owned block
    /// parameter supplies write authority directly). Its establishment is the
    /// block entry, so `block` is the producer identity; the declaration is
    /// owned, never a loan, and gains no borrowed ABI identity by the write.
    /// Every incoming edge moved its whole source into this home, so the
    /// store lands in the destination's own storage, not a predecessor's.
    OwnedBlockParameter {
        block: BlockId,
        parameter: StructuralParameterDeclaration,
    },
}

impl StructuralScalarStoreDestination {
    pub fn place(&self) -> PlaceId {
        match self {
            Self::Parameter(parameter) | Self::OwnedBlockParameter { parameter, .. } => {
                parameter.place
            }
            Self::OwnedResult { result, .. } => result.place,
        }
    }

    pub fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::Parameter(parameter) | Self::OwnedBlockParameter { parameter, .. } => {
                parameter.structural_type
            }
            Self::OwnedResult { result, .. } => result.structural_type,
        }
    }

    pub fn multiplicity(&self) -> StructuralMultiplicity {
        match self {
            Self::Parameter(parameter) | Self::OwnedBlockParameter { parameter, .. } => {
                parameter.multiplicity
            }
            Self::OwnedResult { result, .. } => result.multiplicity,
        }
    }

    /// The borrowed parameter whose loan authorizes the write. Owned homes,
    /// including an owned block parameter, carry their own authority.
    pub fn parameter(&self) -> Option<&StructuralParameterDeclaration> {
        match self {
            Self::Parameter(parameter) => Some(parameter),
            Self::OwnedResult { .. } | Self::OwnedBlockParameter { .. } => None,
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

#[cfg(test)]
mod tests {
    use super::StructuralScalarStoreDestination;
    use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

    #[test]
    fn owned_block_arrival_reports_its_declaration_and_no_loan() {
        let parameter = StructuralParameterDeclaration {
            place: PlaceId::new(11).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(12).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let arrival = StructuralScalarStoreDestination::OwnedBlockParameter {
            block: BlockId::new(3).unwrap(),
            parameter: parameter.clone(),
        };
        assert_eq!(arrival.place(), parameter.place);
        assert_eq!(arrival.structural_type(), parameter.structural_type);
        assert_eq!(arrival.multiplicity(), parameter.multiplicity);
        // The arrival writes with its own authority, never a loan's.
        assert_eq!(arrival.parameter(), None);
        assert_ne!(
            arrival,
            StructuralScalarStoreDestination::Parameter(parameter)
        );
    }
}
