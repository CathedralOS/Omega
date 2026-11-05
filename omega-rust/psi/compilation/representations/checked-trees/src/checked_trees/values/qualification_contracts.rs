//! The requirements a completed source value owes before a loan is formed.
//! Recording the source type's contract establishes no initialized value,
//! membership proof, access authority or ownership claim.

use crate::CheckedStructuralPathQualification;
use language_semantics::SemanticDomainId;
use typed_trees::types::TypeReferenceHandle;

/// Whole-value requirements and descendant paths retain separate subjects.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedStructuralQualificationContract {
    pub qualifications: Vec<SemanticDomainId>,
    pub projected_qualifications: Vec<CheckedStructuralPathQualification>,
    pub domains: Vec<(SemanticDomainId, TypeReferenceHandle)>,
    pub projection_types: Vec<TypeReferenceHandle>,
}

/// Pointer loans carry their supplying contract. Collection descriptors and
/// access profiles have separate publication owners. A failed inventory keeps
/// its reason and cannot be consumed as an empty standing contract.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedReferenceSourceContract {
    SeparateStorage,
    Standing(CheckedStructuralQualificationContract),
    Unavailable(String),
}
