//! Target-neutral structural shapes and qualifications.

mod qualifications;
mod scalar_qualifications;
mod structural;

pub use qualifications::{
    ResultQualificationEstablishment, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralEstablishmentRoute, StructuralPathQualification,
};
pub use scalar_qualifications::{
    ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarFloatRange, ScalarIntegerRange,
    ScalarQualificationCatalog, ScalarQualificationCoercion, ScalarQualificationSet,
};
pub use structural::{
    ByteSequenceCarrier, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};
