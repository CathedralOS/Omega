use language_core::BindingRelevance;
use semantic_vocabulary::{
    IeeeFloatFormat, ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralTypeDeclaration {
    pub id: StructuralTypeId,
    pub identity: String,
    pub shape: StructuralTypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralTypeShape {
    /// A permission carrier retaining the original referent, not an owned copy
    /// of its storage. Owning or moving this value transfers the loan only;
    /// projection through `Referent` must independently respect borrowed access.
    Reference {
        referent: StructuralTypeId,
        access: crate::StructuralAccess,
    },
    /// One whole primitive scalar held behind structural ownership/borrowing
    /// custody. This is a semantic referent shape, not a native layout claim.
    PrimitiveScalar(ScalarType),
    /// One immutable borrowed view over an exact sequence of bytes. The bytes
    /// are semantic payload, not UTF-8 text and not a native pointer/layout.
    ByteSequence(ByteSequenceCarrier),
    /// One immutable borrowed view over a runtime-length sequence of
    /// structural elements. The extent is the view's own stored runtime
    /// length, not part of the type identity; the viewed storage remains
    /// owned by its referent root.
    ElementView { element: StructuralTypeId },
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
    /// A scalar field whose declaration retains an inclusive integer restriction.
    BoundedInteger(semantic_vocabulary::BoundedIntegerType),
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

impl StructuralFieldType {
    /// The scalar carrier used for observation and representation. This does
    /// not discharge restricted-field construction or mutation requirements.
    pub fn scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar_type) => Some(*scalar_type),
            Self::BoundedInteger(integer) => Some(ScalarType::Integer(integer.integer_type())),
            Self::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
            Self::ByteSequence(_) | Self::Structural(_) | Self::Erased { .. } => None,
        }
    }

    /// The declared structural shape a plain leaf field resolves to at a path
    /// end. Bounded leaves keep their restriction out of borrowed shapes, and
    /// erased or structural children are resolved by their own owners.
    ///
    /// An inline byte field resolves to nothing, borrowed-view fields
    /// included. Its extent, capacity, and live length belong to the owning
    /// record declaration, so it has no standalone type identity a consumer
    /// could substitute at a path end: byte_views.md admits a whole byte view
    /// argument only from structural parameters, established literals, or
    /// dominating subslice results, and a byte field reaches a call parameter
    /// only through the inline presentation routes, which keep the owning
    /// root and path. `leaf_copy_shape` is the one place a borrowed-view
    /// field resolves: an explicit leaf copy mints a whole descriptor.
    pub fn canonical_leaf_shape(&self) -> Option<StructuralTypeShape> {
        match self {
            Self::Scalar(scalar_type) => Some(StructuralTypeShape::PrimitiveScalar(*scalar_type)),
            Self::IeeeFloat(format) => Some(StructuralTypeShape::PrimitiveScalar(
                ScalarType::IeeeFloat(*format),
            )),
            Self::ByteSequence(_)
            | Self::BoundedInteger(_)
            | Self::Structural(_)
            | Self::Erased { .. } => None,
        }
    }

    /// The shape an explicit `StructuralLeafCopy` of this field mints. It is
    /// `canonical_leaf_shape` plus the borrowed-view descriptor: copying a
    /// `&[u8]` field's descriptor out of the parent yields a whole borrowed
    /// view, whereas a path ending at that field never stands for one.
    pub fn leaf_copy_shape(&self) -> Option<StructuralTypeShape> {
        match self {
            Self::ByteSequence(ByteSequenceCarrier::BorrowedView) => Some(
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
            ),
            other => other.canonical_leaf_shape(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ByteSequenceCarrier {
    BorrowedView,
    BoundedOwned { capacity: u64 },
}
