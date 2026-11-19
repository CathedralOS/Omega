// We describe the backing storage interpreted by a native placed descriptor.
// A resident owner lends its allocation rather than the bytes of its Extent
// record, and a successful `View` boundary already holds the backing pointer
// (`BorrowedExtent`) so address derivation never emits a second load from an
// Extent record.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{AccessRamCorrespondence, ResidentReferenceSource, StructuralTypeShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessReferenceBacking {
    OrdinaryRam(AccessRamCorrespondence),
    Resident(ResidentReferenceBacking),
    BorrowedExtent(BorrowedExtentBacking), // Produced only by the successful leaf of a checked View boundary.
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

impl AccessReferenceBacking {
    pub fn address_source(
        &self,
        source: &terminal_psi::StructuralArgument,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<terminal_psi::StructuralArgument> {
        let mut address = source.clone();
        let extent = match self {
            Self::OrdinaryRam(_) => None,
            Self::Resident(backing) => Some((backing.owner, backing.roles.base)),
            Self::BorrowedExtent(_) => return None,
        };
        if let Some((owner, base)) = extent {
            let declaration = terminal_psi::find_structural_type(types, owner)?;
            let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
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
