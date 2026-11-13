// Closed structural identity of executable code supplied outside Omega, including foreign locators and evaluated import/syscall receipts.

use crate::record::PackageReviewTypeIdentity;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewForeignLocator {
    PeByName {
        library: Vec<u8>,
        export: Vec<u8>,
    },
    PeByOrdinal {
        library: Vec<u8>,
        ordinal: u16,
    },
    ElfVersioned {
        object: Vec<u8>,
        symbol: Vec<u8>,
        version: Vec<u8>,
    },
    MachODylibSymbol {
        install_name: Vec<u8>,
        symbol: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableParameter {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewExternalCallableParameter {
    field_projection_accessors!(borrowed type_identity: &PackageReviewTypeIdentity);
    field_projection_accessors!(copied is_const: bool, is_mutable: bool);
}
