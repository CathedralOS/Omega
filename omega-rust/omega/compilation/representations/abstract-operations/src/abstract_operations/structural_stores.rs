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

/// One store-destination field accessor: every carrying variant reads the
/// same field through its parameter or result declaration.
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
