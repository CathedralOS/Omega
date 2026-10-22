//! The backing interpreted by a native placed descriptor.
//!
//! A resident owner lends its allocation, not the bytes of its Extent record.
//! Keep that distinction with the ordinary source operand through every native
//! representation. These rows describe address derivation; the source loan,
//! successful lifecycle and admitted provider still supply custody and authority.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{PlacedRamCorrespondence, ResidentReferenceSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacedReferenceBacking {
    OrdinaryRam(PlacedRamCorrespondence),
    Resident(ResidentReferenceBacking),
    /// Produced only by the successful leaf of a checked View boundary.
    /// This is not an unconditional EstablishPlacedReference operation.
    BorrowedExtent(BorrowedExtentBacking),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedExtentBacking {
    pub owner: StructuralTypeId,
    pub schema: StructuralTypeId,
    pub carrier: StructuralTypeId,
    pub base: semantic_vocabulary::StructuralFieldId,
    pub length: semantic_vocabulary::StructuralFieldId,
    pub granted: semantic_vocabulary::StructuralDomainId,
    pub placement_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidentReferenceBacking {
    pub owner: StructuralTypeId,
    pub schema: StructuralTypeId,
    pub carrier: StructuralTypeId,
    pub roles: ResidentReferenceSource,
    pub placement_commitment: [u8; 32],
}

impl PlacedReferenceBacking {
    /// The address-bearing endpoint, separately from the semantic loan owner.
    /// Selecting this field does not lend arbitrary address values: consumers
    /// must keep the original source operand and its admitted owner custody.
    pub fn address_source(
        &self,
        source: &terminal_psi::StructuralArgument,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<terminal_psi::StructuralArgument> {
        let mut address = source.clone();
        let extent = match self {
            Self::OrdinaryRam(_) => None,
            Self::Resident(backing) => Some((backing.owner, backing.roles.base)),
            // The successful View already contains the backing pointer. Its
            // interpretation must not emit a second load from an Extent record.
            Self::BorrowedExtent(_) => return None,
        };
        if let Some((owner, base)) = extent {
            let declaration = terminal_psi::find_structural_type(types, owner)?;
            let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape
            else {
                return None;
            };
            let field = fields.iter().find(|field| field.id == base)?;
            address
                .path
                .push(terminal_psi::StructuralPathSegment::Field(field.identity));
        }
        Some(address)
    }

    pub fn source_type(&self) -> StructuralTypeId {
        match self {
            Self::OrdinaryRam(backing) => backing.schema,
            Self::Resident(backing) => backing.owner,
            Self::BorrowedExtent(backing) => backing.owner,
        }
    }

    pub fn schema(&self) -> StructuralTypeId {
        match self {
            Self::OrdinaryRam(backing) => backing.schema,
            Self::Resident(backing) => backing.schema,
            Self::BorrowedExtent(backing) => backing.schema,
        }
    }

    pub fn carrier(&self) -> StructuralTypeId {
        match self {
            Self::OrdinaryRam(backing) => backing.carrier,
            Self::Resident(backing) => backing.carrier,
            Self::BorrowedExtent(backing) => backing.carrier,
        }
    }

    pub fn resident_source(&self) -> Option<ResidentReferenceSource> {
        match self {
            Self::OrdinaryRam(_) | Self::BorrowedExtent(_) => None,
            Self::Resident(backing) => Some(backing.roles),
        }
    }
}
