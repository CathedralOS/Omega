//! Structural types, domains, qualifications and quotients.

pub mod descriptor_locations;
mod opaque_storage;
mod qualifications;
mod queries;
pub mod quotients;
pub mod representation_views;
mod structural;
pub mod structural_path_encoding;
pub mod values;

pub use opaque_storage::OpaqueStorageShapes;
pub use qualifications::*;
pub use structural::*;

use crate::{TerminalIdentityFormations, TerminalMathematicalCarrier};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeDeclarations {
    pub scalar_qualifications: ScalarQualificationCatalog,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_referents: Vec<StructuralReferentIdentity>,
    pub structural_domains: Vec<StructuralDomainDeclaration>,
    pub structural_qualification_changes: Vec<StructuralQualificationChange>,
    pub opaque_representations: Vec<crate::TerminalOpaqueRepresentation>,
    pub mathematical_carriers: Vec<TerminalMathematicalCarrier>,
    pub identity_formations: TerminalIdentityFormations,
    pub quotients: quotients::TerminalQuotientCatalog,
}
