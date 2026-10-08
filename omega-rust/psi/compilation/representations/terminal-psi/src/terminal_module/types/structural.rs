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
        storage: Option<semantic_vocabulary::AuthoredRecordStorage>,
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
    pub storage: Option<semantic_vocabulary::AuthoredFieldStorage>,
}

/// The field carrying `identity` on a declaration roster, live or erased.
pub fn field_by_identity<'fields>(
    fields: &'fields [StructuralFieldDeclaration],
    identity: &str,
) -> Option<&'fields StructuralFieldDeclaration> {
    fields.iter().find(|field| field.identity == identity)
}

/// The field carrying `identity` on a declaration roster, when it is not
/// erased.
pub fn live_field_by_identity<'fields>(
    fields: &'fields [StructuralFieldDeclaration],
    identity: &str,
) -> Option<&'fields StructuralFieldDeclaration> {
    fields
        .iter()
        .find(|field| field.identity == identity && !field.relevance.is_erased())
}

/// The one declaration carrying `id` — a cycle through `visiting` or an
/// absent/ambiguous match means the caller's walk does not own this node.
pub fn unique_structural_declaration<'d>(
    declarations: &'d [StructuralTypeDeclaration],
    id: StructuralTypeId,
    visiting: &[StructuralTypeId],
) -> Option<&'d StructuralTypeDeclaration> {
    if visiting.contains(&id) {
        return None;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == id);
    let declaration = matches.next()?;
    matches.next().is_none().then_some(declaration)
}

impl StructuralTypeDeclaration {
    /// Whether this declaration names a record shape.
    pub fn is_record_shape(&self) -> bool {
        matches!(self.shape, StructuralTypeShape::Record { .. })
    }

    /// Whether this declaration names a bounded owned byte-sequence carrier.
    pub fn is_bounded_owned_byte_sequence(&self) -> bool {
        matches!(
            self.shape,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
        )
    }
}

impl StructuralTypeShape {
    /// Whether this shape composes other structural types by value: record,
    /// fixed array, sum, or mixed interiors.
    pub fn is_composite_shape(&self) -> bool {
        matches!(
            self,
            StructuralTypeShape::Record { .. }
                | StructuralTypeShape::FixedArray { .. }
                | StructuralTypeShape::Sum { .. }
                | StructuralTypeShape::Mixed { .. }
        )
    }

    /// Whether this shape carries bytes behind an immutable borrowed view.
    pub fn is_borrowed_view_byte_sequence(&self) -> bool {
        matches!(
            self,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        )
    }
}

impl StructuralTypeShape {
    /// The fields addressed directly on the carrier, without a `Case` step: a
    /// record's fields, or a mixed shape's common fields, which sit at one
    /// offset in every case.
    pub fn direct_fields(&self) -> Option<&[StructuralFieldDeclaration]> {
        match self {
            Self::Record { fields, .. } | Self::Mixed { fields, .. } => Some(fields),
            _ => None,
        }
    }

    /// The field roster of a record shape.
    pub fn record_fields(&self) -> Option<&[StructuralFieldDeclaration]> {
        match self {
            Self::Record { fields, .. } => Some(fields),
            _ => None,
        }
    }

    /// The unique live field `identity` names on a record shape — absent
    /// when the name is missing, erased, or resolves to more than one id.
    pub fn live_record_field(&self, identity: &str) -> Option<&StructuralFieldDeclaration> {
        live_field(self.record_fields()?, identity)
    }

    /// The case roster of a sum or mixed shape.
    pub fn sum_cases(&self) -> Option<&[StructuralCaseDeclaration]> {
        match self {
            Self::Sum { cases } | Self::Mixed { cases, .. } => Some(cases),
            _ => None,
        }
    }

    /// Whether this shape is a bounded owned `[u8; N]` carrier.
    pub const fn is_bounded_owned_byte_sequence(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
        )
    }

    /// Whether this shape is a borrowed `[u8]` view readable through shared
    /// access: an explicit shared borrow or no bound access at all.
    pub const fn is_shared_readable_byte_view(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(carrier) if carrier.is_shared_readable_view()
        )
    }

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
            Self::ByteSequence(carrier) if carrier.supports_result_qualifications()
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

    /// Whether this carrier is a borrowed byte view readable through shared
    /// access: an explicit shared borrow or no bound access at all.
    pub const fn is_shared_readable_view(&self) -> bool {
        matches!(
            self,
            Self::BorrowedView {
                access: None | Some(crate::StructuralAccess::SharedBorrow),
            }
        )
    }

    /// Whether root qualifications may travel on this carrier — a bounded
    /// owner or a shared-readable view, as on the shape predicate.
    pub const fn supports_result_qualifications(&self) -> bool {
        matches!(
            self,
            Self::BoundedOwned { .. }
                | Self::BorrowedView {
                    access: None | Some(crate::StructuralAccess::SharedBorrow),
                }
        )
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
    /// The structural type identity, if this field is structural.
    pub fn as_structural(&self) -> Option<StructuralTypeId> {
        let StructuralFieldType::Structural(structural) = self else {
            return None;
        };
        Some(*structural)
    }

    /// The scalar type, if this field is a plain scalar.
    pub fn as_scalar(&self) -> Option<ScalarType> {
        let StructuralFieldType::Scalar(scalar) = self else {
            return None;
        };
        Some(*scalar)
    }
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

/// The structural children a field roster names: each `Structural` field's
/// declared type in roster order — the edges a type-graph walk follows.
pub fn structural_children(
    fields: &[StructuralFieldDeclaration],
) -> impl Iterator<Item = StructuralTypeId> + '_ {
    fields.iter().filter_map(|field| match field.field_type {
        StructuralFieldType::Structural(child) => Some(child),
        _ => None,
    })
}

/// The unique live field `identity` names on a field roster — absent when
/// the name is missing, erased, or ambiguous.
pub fn live_field<'a>(
    fields: &'a [StructuralFieldDeclaration],
    identity: &str,
) -> Option<&'a StructuralFieldDeclaration> {
    let mut matching = fields.iter().filter(|field| field.identity == identity);
    let field = matching.next()?;
    if matching.next().is_some()
        || field.relevance.is_erased()
        || fields
            .iter()
            .filter(|candidate| candidate.id == field.id)
            .count()
            != 1
    {
        return None;
    }
    Some(field)
}

/// The structural type declaration keyed `id` in a declaration roster.
pub fn find_structural_type<'types>(
    types: &'types [StructuralTypeDeclaration],
    id: StructuralTypeId,
) -> Option<&'types StructuralTypeDeclaration> {
    types.iter().find(|declaration| declaration.id == id)
}

/// The field keyed `id` on a declaration roster, when it is not erased.
pub fn live_field_by_id<'fields>(
    fields: &'fields [StructuralFieldDeclaration],
    id: StructuralFieldId,
) -> Option<&'fields StructuralFieldDeclaration> {
    fields
        .iter()
        .find(|field| field.id == id && !field.relevance.is_erased())
}

/// The live-field position and declaration `identity` names on a field
/// roster — absent when no live field carries it. Position counts live
/// fields only, so callers can index a parallel live-only layout roster.
pub fn live_field_entry<'fields>(
    fields: &'fields [StructuralFieldDeclaration],
    identity: &str,
) -> Option<(usize, &'fields StructuralFieldDeclaration)> {
    fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .enumerate()
        .find(|(_, field)| field.identity == identity)
}

/// The structural case declaration keyed `id` in a case roster.
pub fn find_structural_case<'cases>(
    cases: &'cases [StructuralCaseDeclaration],
    id: StructuralCaseId,
) -> Option<&'cases StructuralCaseDeclaration> {
    cases.iter().find(|case| case.id == id)
}

/// The position of the case keyed `id` in `cases`, when declared. Position
/// encodes the wire ordinal, so selection and aggregate lowering index it
/// directly rather than re-deriving it from the declaration.
pub fn structural_case_position(
    cases: &[StructuralCaseDeclaration],
    id: StructuralCaseId,
) -> Option<usize> {
    cases.iter().position(|case| case.id == id)
}
