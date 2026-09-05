use psi_core::{
    IeeeFloatFormat, ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use psi_language_core::BindingRelevance;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralTypeDeclaration {
    pub id: StructuralTypeId,
    pub identity: String,
    pub shape: StructuralTypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralTypeShape {
    /// One whole primitive scalar held behind structural ownership/borrowing
    /// custody. This is a semantic referent shape, not a native layout claim.
    PrimitiveScalar(ScalarType),
    /// One immutable borrowed view over an exact sequence of bytes. The bytes
    /// are semantic payload, not UTF-8 text and not a native pointer/layout.
    ByteSequence(ByteSequenceCarrier),
    Record {
        /// Declaration order is semantic. Field IDs must nevertheless be
        /// strictly increasing so the same record has one canonical spelling.
        fields: Vec<StructuralFieldDeclaration>,
    },
    FixedArray {
        element: StructuralTypeId,
        length: u64,
    },
    /// A closed pure sum. Case and payload-field declaration order is semantic;
    /// their IDs are strictly increasing in the canonical encoding.
    Sum {
        cases: Vec<StructuralCaseDeclaration>,
    },
    /// A closed sum with fields available independently of the selected case.
    /// Common-field and case declaration order is semantic and all IDs are
    /// canonical within their respective namespaces.
    Mixed {
        fields: Vec<StructuralFieldDeclaration>,
        cases: Vec<StructuralCaseDeclaration>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralCaseDeclaration {
    pub id: StructuralCaseId,
    pub identity: String,
    pub fields: Vec<StructuralFieldDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralFieldDeclaration {
    pub id: StructuralFieldId,
    pub identity: String,
    /// Authored semantic relevance. Erased rows remain in terminal identity and
    /// proof structure even though Omega omits them from native layout.
    pub relevance: BindingRelevance,
    pub field_type: StructuralFieldType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralFieldType {
    Scalar(ScalarType),
    /// Relevant IEEE leaf retained for structural identity and predicates.
    IeeeFloat(IeeeFloatFormat),
    ByteSequence(ByteSequenceCarrier),
    Structural(StructuralTypeId),
    /// Exact semantic type identity for an erased field whose carrier need not
    /// belong to the executable structural/layout vocabulary.
    Erased {
        type_identity: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ByteSequenceCarrier {
    BorrowedView,
    BoundedOwned { capacity: u64 },
}
