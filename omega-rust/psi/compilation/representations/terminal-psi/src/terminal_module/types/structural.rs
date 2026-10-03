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
    /// An established routed binding with no runtime payload under selected
    /// Fused realization. This semantic carrier is not constructible by a
    /// record literal or zero initialization. Ordinary loans preserve its
    /// occurrence; entry provisioning retains separate establishment evidence.
    FusedServiceBinding,
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

impl StructuralTypeShape {
    /// Whether this shape is a borrowed `[u8]` view, whatever access the view
    /// carries. Most readers ask only that -- the descriptor geometry is the
    /// same for a shared and an exclusive view -- and comparing against a
    /// constructed carrier would silently answer no for the other access.
    pub const fn is_borrowed_byte_view(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        )
    }

    /// Whether the carrier borrows caller-owned storage — the access-bearing
    /// descriptors (`BorrowedView`, `BoundedView`). This answers custody, not
    /// descriptor layout: a bounded view's descriptor is a bare pointer with
    /// no stored length, so consumers that read one must not use this.
    pub const fn is_borrowed_byte_carrier(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(
                ByteSequenceCarrier::BorrowedView { .. } | ByteSequenceCarrier::BoundedView { .. }
            )
        )
    }

    /// Root qualifications travel on a byte carrier — a bounded owner or a
    /// shared view. The view's access binds on the field, parameter, or
    /// result that declares it, so the type-registered shape carries none
    /// and counts the same as an explicitly shared one; the declaring
    /// member's own access check decides mutability.
    pub const fn supports_result_qualifications(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(
                ByteSequenceCarrier::BoundedOwned { .. }
                    | ByteSequenceCarrier::BorrowedView {
                        access: None | Some(crate::StructuralAccess::SharedBorrow),
                    }
            )
        )
    }
}

impl ByteSequenceCarrier {
    /// Declaration predicates describe owned bytes or an immutable shared view.
    /// Construction and mutation must independently preserve those predicates.
    pub const fn supports_field_predicates(&self) -> bool {
        matches!(
            self,
            Self::BoundedOwned { .. }
                | Self::BorrowedView {
                    access: Some(crate::StructuralAccess::SharedBorrow),
                }
        )
    }

    /// Whether this carrier reads through borrowed storage at all.
    pub const fn is_borrowed(&self) -> bool {
        matches!(self, Self::BorrowedView { .. } | Self::BoundedView { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralFieldType {
    Scalar(ScalarType),
    /// Dedicated atomic storage. Its scalar representation does not authorize
    /// ordinary shared mutation or a plain scalar reference to the cell.
    Atomic(ScalarType),
    /// A scalar field whose declaration retains an inclusive integer restriction.
    BoundedInteger(semantic_vocabulary::BoundedIntegerType),
    /// A scalar carrier with a closed declaration predicate. Construction and
    /// every replacement prove this predicate; a validated read retains it on
    /// its fresh snapshot. The predicate grants no raw-reference permission.
    RefinedScalar(semantic_vocabulary::ScalarRefinement),
    /// Relevant IEEE leaf retained for structural identity and predicates.
    IeeeFloat(IeeeFloatFormat),
    ByteSequence(ByteSequenceCarrier),
    /// Declaration-local byte restrictions; layout remains the carrier layout.
    PredicateByteSequence {
        carrier: ByteSequenceCarrier,
        predicates: crate::ByteSequencePredicateSet,
    },
    Structural(StructuralTypeId),
    /// Exact semantic type identity for an erased field whose carrier need not
    /// belong to the executable structural/layout vocabulary.
    Erased {
        type_identity: String,
    },
}

impl StructuralFieldType {
    /// The admitted fixed-width atomic carrier, never an ordinary scalar of
    /// the same representation. Address and floating carriers are not cells.
    pub fn atomic_scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Atomic(ScalarType::Boolean) => Some(ScalarType::Boolean),
            Self::Atomic(ScalarType::Integer(integer))
                if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
            {
                Some(ScalarType::Integer(*integer))
            }
            _ => None,
        }
    }

    pub const fn byte_sequence_carrier(&self) -> Option<ByteSequenceCarrier> {
        match self {
            Self::ByteSequence(carrier) | Self::PredicateByteSequence { carrier, .. } => {
                Some(*carrier)
            }
            _ => None,
        }
    }

    pub const fn byte_predicates(&self) -> crate::ByteSequencePredicateSet {
        match self {
            Self::PredicateByteSequence { predicates, .. } => *predicates,
            _ => crate::ByteSequencePredicateSet::EMPTY,
        }
    }

    /// The scalar carrier used for observation and representation. This does
    /// not discharge restricted-field construction or mutation requirements.
    pub fn scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar_type) | Self::Atomic(scalar_type) => Some(*scalar_type),
            Self::BoundedInteger(integer) => Some(ScalarType::Integer(integer.integer_type())),
            Self::RefinedScalar(refinement) => Some(refinement.carrier()),
            Self::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
            Self::ByteSequence(_)
            | Self::PredicateByteSequence { .. }
            | Self::Structural(_)
            | Self::Erased { .. } => None,
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
            | Self::PredicateByteSequence { .. }
            | Self::BoundedInteger(_)
            | Self::RefinedScalar(_)
            | Self::Atomic(_)
            | Self::Structural(_)
            | Self::Erased { .. } => None,
        }
    }

    /// The shape an explicit `StructuralLeafCopy` of this field mints. It is
    /// `canonical_leaf_shape` plus explicit byte snapshots: copying a borrowed
    /// view yields its descriptor, while a bounded owner yields independent
    /// live bytes and length. This resolves storage only; copied predicates
    /// still require independent evidence at their consuming operation.
    pub fn leaf_copy_shape(&self) -> Option<StructuralTypeShape> {
        match self.byte_sequence_carrier() {
            Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
                Some(StructuralTypeShape::ByteSequence(carrier))
            }
            // The copy yields a whole borrowed view TYPE, whose identity
            // peels the reference shell, so the field's own access does not
            // travel with it — nor do its byte predicates, which the
            // consuming operation proves independently.
            Some(ByteSequenceCarrier::BorrowedView { .. }) => {
                Some(StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView { access: None },
                ))
            }
            _ => self.canonical_leaf_shape(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ByteSequenceCarrier {
    /// A `&'r [u8]` or `&'r mut [u8]` descriptor.
    ///
    /// `access` is the view's access where one binds, and `None` where none
    /// does. A record field binds one: independent verification has nothing
    /// else to read a byte store's authority from, and without it a shared and
    /// an exclusive view field are the same carrier after checking. A
    /// standalone view type binds none -- its identity deliberately peels the
    /// reference shell, so `&mut [u8]` and `&write [u8]` share one identity and
    /// could not share one carrier if the access rode inside it.
    BorrowedView {
        access: Option<crate::terminal_module::ownership::StructuralAccess>,
    },
    /// A `&'r [u8; N]` or `&'r mut [u8; N]` reference to fixed storage. The
    /// extent is the declaration's literal length, not a stored descriptor
    /// field, so `capacity` rides the carrier and the descriptor is a bare
    /// referent pointer.
    BoundedView {
        access: Option<crate::terminal_module::ownership::StructuralAccess>,
        capacity: u64,
    },
    BoundedOwned {
        capacity: u64,
    },
}
