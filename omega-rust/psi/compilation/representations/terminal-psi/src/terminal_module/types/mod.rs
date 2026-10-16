//! Target-neutral structural shapes and qualifications.

mod qualifications;
mod scalar_qualifications;
mod structural;

pub use qualifications::{
    ResultQualificationEstablishment, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralDomainPredicate, StructuralEstablishmentRoute, StructuralPathQualification,
};
pub use scalar_qualifications::{
    ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarFloatRange, ScalarIntegerRange,
    ScalarQualificationCatalog, ScalarQualificationCoercion, ScalarQualificationSet,
};
pub use structural::{
    ByteSequenceCarrier, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralReferentIdentity, StructuralTypeDeclaration,
    StructuralTypeShape, field_by_identity, find_structural_case, find_structural_type, live_field,
    live_field_by_id, live_field_by_identity, live_field_entry, structural_case_by_identity,
    structural_case_initializer_path, structural_case_position, structural_children,
    structural_type_by_identity, structural_type_contains_reference,
    structural_type_contains_reference_by, structural_type_has_borrowed_storage_by,
    unique_structural_declaration,
};
