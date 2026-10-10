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
    /// An exact entry ABI slot, owned or borrowed, or a non-entry address
    /// join carrying a loan. The retained access distinguishes owned bytes
    /// from referent authority; writing never changes that access.
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
            Self::Parameter(parameter)
                if parameter.access != terminal_psi::StructuralAccess::Owned =>
            {
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

/// Restoration windows retain the same storage declarations as ordinary
/// stores. A continuation establishes an owned home just as an operation does;
/// transferring a reference carrier there does not erase its loan. Window
/// admission separately authenticates referent authority and restoration debt.
pub type StructuralWindowRoot = StructuralScalarStoreDestination;
