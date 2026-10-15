use crate::terminal_module::TerminalIdentityIndex;
use language_core::BindingRelevance;
use semantic_vocabulary::{
    IeeeFloatFormat, ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralTypeDeclaration {
    /// Nominal opacity marker independent of the retained physical shape.
    /// Every marked declaration requires its exact formation catalog row.
    pub quotient_carrier: Option<StructuralTypeId>,
    pub id: StructuralTypeId,
    pub identity: TerminalIdentityIndex,
    /// Checked authority that disposing represented owned contents requires no
    /// executable cleanup. This does not remove claims, loans, or access checks.
    pub no_code_affine_cleanup: bool,
    /// Checked authority to retire contained reference descriptors without
    /// observing old payloads or executing nominal cleanup. Exact live
    /// custody and child closure remain obligations of each replacement.
    pub descriptor_cleanup: bool,
    pub shape: StructuralTypeShape,
    /// Closed scalar declaration predicate, independent from physical carrier
    /// geometry. Only BoundedInteger and RefinedScalar are admitted, and only
    /// on PrimitiveScalar. Construction and replacement establish it on their
    /// exact SSA operand; reads retain it on their fresh snapshot. The type
    /// identity distinguishes restricted and unrestricted scalar declarations.
    pub scalar_restriction: Option<StructuralFieldType>,
}

/// A closed referent identity with no demanded storage definition. This is not
/// a shape: it cannot establish owned storage, fields, or dereference access.
/// Reference transport needs exact identity, not the transitive storage catalog
/// of every type that could be reached by following a pointer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralReferentIdentity {
    pub id: StructuralTypeId,
    /// Canonical closed-type export payload. Runtime relationships use `id`,
    /// never a spelling comparison or a reconstructed source application.
    pub identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralTypeShape {
    /// A logical routed binding, independent of its provider's storage mode.
    /// It is not constructible by a record literal or zero initialization.
    /// Ordinary loans preserve its occurrence; realization must establish
    /// the binding before choosing its storage. The marker grants no authority.
    BindingCarrier,
    /// A permission carrier retaining the original referent, not an owned copy
    /// of its storage. Owning or moving this value transfers the loan only;
    /// projection through `Referent` must independently respect borrowed access.
    /// The target may name an identity-only referent until an operation demands
    /// its storage definition. Pointer transport never supplies that definition.
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
    ElementView {
        element: StructuralTypeId,
        /// Stored descriptor types retain their declared backing ceiling.
        /// Canonical whole descriptors use occurrence custody instead. Owning
        /// a container never upgrades the permission of its stored views.
        access: Option<crate::StructuralAccess>,
    },
    Record {
        /// Declaration order is semantic. Field IDs must nevertheless be
        /// strictly increasing so the same record has one canonical spelling.
        fields: Vec<StructuralFieldDeclaration>,
        storage: crate::StructuralRecordStorage,
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
    pub identity: TerminalIdentityIndex,
    pub fields: Vec<StructuralFieldDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralFieldDeclaration {
    pub id: StructuralFieldId,
    pub identity: TerminalIdentityIndex,
    /// Authored semantic relevance. Erased rows remain in terminal identity and
    /// proof structure even though Omega omits them from native layout.
    pub relevance: BindingRelevance,
    pub field_type: StructuralFieldType,
    pub storage: Option<semantic_vocabulary::AuthoredFieldStorage>,
}

/// The field carrying `identity` on a declaration roster, live or erased.
pub fn field_by_identity<'fields>(
    fields: &'fields [StructuralFieldDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&'fields StructuralFieldDeclaration> {
    fields.iter().find(|field| field.identity == identity)
}

/// The field carrying `identity` on a declaration roster, when it is not
/// erased.
pub fn live_field_by_identity<'fields>(
    fields: &'fields [StructuralFieldDeclaration],
    identity: TerminalIdentityIndex,
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
            crate::record_or_fixed_array_shape_pattern!() | crate::sum_or_mixed_shape_pattern!()
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
    pub fn live_record_field(
        &self,
        identity: TerminalIdentityIndex,
    ) -> Option<&StructuralFieldDeclaration> {
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
            Self::ByteSequence(crate::byte_sequence_view_carrier_pattern!())
        )
    }

    /// Byte-predicate observations use bounded owners or shared-readable views.
    /// This is not a qualification whitelist: other carriers can retain their
    /// checked domains without participating in byte-predicate evaluation.
    /// An unbound view shape binds access on its field, parameter or result;
    /// that declaration's access judgment still decides mutability.
    pub const fn is_owned_or_shared_byte_carrier(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(carrier) if carrier.is_owned_or_shared_byte_carrier()
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

    /// A bounded owner or shared-readable view supports immutable byte
    /// observations; this says nothing about other carriers' qualifications.
    pub const fn is_owned_or_shared_byte_carrier(&self) -> bool {
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
        type_identity: TerminalIdentityIndex,
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

    /// The complete leaf contract of an exact storage projection. A bounded
    /// scalar may name a standalone referent only with its declaration predicate
    /// intact; choosing a declaration by physical shape would erase that bound.
    /// Atomic cells and inline byte descriptors retain their separate protocols.
    pub fn canonical_leaf_type(&self) -> Option<Self> {
        match self {
            Self::BoundedInteger(_) | Self::RefinedScalar(_) => Some(self.clone()),
            _ => self.canonical_leaf_shape().and_then(Self::from_leaf_shape),
        }
    }

    pub fn shared_observation_type(&self) -> Option<Self> {
        self.shared_observation_shape()
            .and_then(Self::from_leaf_shape)
    }

    /// A nonescaping call loan presents the existing field's carrier, not a
    /// detached value with its predicate erased. Psi checks writable handback
    /// against that predicate; geometry consumers preserve its original path.
    /// Shared view descriptors keep their actual two-word presentation.
    pub fn borrowed_presentation_type(
        &self,
        access: crate::StructuralAccess,
        expected: Option<&StructuralTypeDeclaration>,
    ) -> Option<Self> {
        if access != crate::StructuralAccess::Owned
            && let Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) =
                self.byte_sequence_carrier()
            && let Some(expected) = expected
            && expected.shape == StructuralTypeShape::ByteSequence(carrier)
        {
            // The call explicitly supplies this presentation. The field keeps
            // its own storage identity and predicates; equal-shaped catalog
            // entries cannot choose a nominal identity on its behalf.
            return Some(Self::Structural(expected.id));
        }
        if expected.is_some_and(|expected| expected.scalar_restriction.as_ref() == Some(self)) {
            return self.canonical_leaf_type();
        }
        if access == crate::StructuralAccess::SharedBorrow {
            return match self.byte_sequence_carrier() {
                Some(ByteSequenceCarrier::BorrowedView {
                    access: Some(crate::StructuralAccess::SharedBorrow),
                }) => Some(Self::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                })),
                _ => self.shared_observation_type(),
            };
        }
        if access == crate::StructuralAccess::Owned {
            return None;
        }
        match self {
            Self::BoundedInteger(_) | Self::RefinedScalar(_) => self.shared_observation_type(),
            _ => self.canonical_leaf_type(),
        }
    }

    pub fn leaf_copy_type(&self) -> Option<Self> {
        self.leaf_copy_shape().and_then(Self::from_leaf_shape)
    }

    fn from_leaf_shape(shape: StructuralTypeShape) -> Option<Self> {
        match shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(Self::Scalar(scalar)),
            StructuralTypeShape::ByteSequence(carrier) => Some(Self::ByteSequence(carrier)),
            _ => None,
        }
    }

    /// Resolve a complete leaf contract against one unique retained declaration.
    /// Equal carrier widths never substitute a plain scalar or another range.
    pub fn leaf_declaration<'a>(
        &self,
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    ) -> Option<&'a StructuralTypeDeclaration> {
        let (shape, restriction) = match self {
            Self::BoundedInteger(_) | Self::RefinedScalar(_) => (
                StructuralTypeShape::PrimitiveScalar(self.scalar_type()?),
                Some(self),
            ),
            Self::ByteSequence(carrier) => (StructuralTypeShape::ByteSequence(*carrier), None),
            _ => (self.canonical_leaf_shape()?, None),
        };
        let mut matching = declarations.filter(|declaration| {
            declaration.shape == shape && declaration.scalar_restriction.as_ref() == restriction
        });
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
    }

    /// The carrier a shared observation of the original field presents.
    /// Restrictions stay on the owning field: this grants neither a standalone
    /// owned leaf nor write access. Atomic fields retain their separate access
    /// protocol and cannot supply an ordinary scalar loan.
    pub fn shared_observation_shape(&self) -> Option<StructuralTypeShape> {
        match self {
            Self::BoundedInteger(integer) => Some(StructuralTypeShape::PrimitiveScalar(
                ScalarType::Integer(integer.integer_type()),
            )),
            Self::RefinedScalar(refinement) => {
                Some(StructuralTypeShape::PrimitiveScalar(refinement.carrier()))
            }
            _ => self.canonical_leaf_shape(),
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
    identity: TerminalIdentityIndex,
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

/// Whether the type graph under `root` reaches a reference leaf. Owned
/// containment only: a reference's referent is not its payload. The walk
/// follows declaration-order children through record, sum and mixed fields
/// and through fixed-array and element-view elements; `visited` bounds it
/// against shape cycles.
pub fn structural_type_contains_reference(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
    structural_type_contains_reference_by(|id| find_structural_type(types, id), root)
}

/// The same walk over a caller-chosen declaration lookup, for consumers that
/// hold declarations keyed by id rather than in declaration-order slices.
pub fn structural_type_contains_reference_by<'types>(
    lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    structural_type_contains_by(
        lookup,
        root,
        |shape| matches!(shape, StructuralTypeShape::Reference { .. }),
        |_| false,
    )
}

/// Whether returned storage needs an origin contract for any reference or
/// borrowed view. Inline byte-view fields participate even without a child
/// type declaration; an owned byte array has no such external origin.
pub fn structural_type_has_borrowed_storage_by<'types>(
    lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    structural_type_contains_by(
        lookup,
        root,
        |shape| {
            matches!(
                shape,
                StructuralTypeShape::Reference { .. }
                    | StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(
                        ByteSequenceCarrier::BorrowedView { .. }
                            | ByteSequenceCarrier::BoundedView { .. }
                    )
            )
        },
        |field| {
            matches!(
                field.byte_sequence_carrier(),
                Some(
                    ByteSequenceCarrier::BorrowedView { .. }
                        | ByteSequenceCarrier::BoundedView { .. }
                )
            )
        },
    )
}

fn structural_type_contains_by<'types>(
    lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    root: StructuralTypeId,
    contains_shape: impl Fn(&StructuralTypeShape) -> bool,
    contains_field: impl Fn(&StructuralFieldType) -> bool,
) -> bool {
    let mut pending = vec![root];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = lookup(current) else {
            continue;
        };
        if contains_shape(&declaration.shape) {
            return true;
        }
        let mut push_fields = |fields: &[StructuralFieldDeclaration]| {
            pending.extend(structural_children(fields));
            fields
                .iter()
                .any(|field| !field.relevance.is_erased() && contains_field(&field.field_type))
        };
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => {
                if push_fields(fields) {
                    return true;
                }
            }
            StructuralTypeShape::Sum { cases } => {
                if cases.iter().any(|case| push_fields(&case.fields)) {
                    return true;
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                if push_fields(fields) || cases.iter().any(|case| push_fields(&case.fields)) {
                    return true;
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                // An empty carrier stores no element payload.
                if *length != 0 {
                    pending.push(*element);
                }
            }
            StructuralTypeShape::ElementView { element, .. } => pending.push(*element),
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::BindingCarrier
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_) => {}
        }
    }
    false
}

/// The case declaration carrying `identity` on a declaration roster.
pub fn structural_case_by_identity<'cases>(
    cases: &'cases [StructuralCaseDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&'cases StructuralCaseDeclaration> {
    cases.iter().find(|case| case.identity == identity)
}

/// The type declaration carrying `identity` on a declaration roster.
pub fn structural_type_by_identity<'types>(
    types: &'types [StructuralTypeDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&'types StructuralTypeDeclaration> {
    types
        .iter()
        .find(|declaration| declaration.identity == identity)
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
    identity: TerminalIdentityIndex,
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

/// The logical destination of one declaration-ordered case initializer.
/// Common and payload field IDs are separate namespaces: the roster ordinal
/// disambiguates them. Common children use Field alone; payload custody and
/// descriptor origins retain Case/Field even when their physical bytes overlay.
pub fn structural_case_initializer_path(
    shape: &StructuralTypeShape,
    selected_case: StructuralCaseId,
    field_ordinal: usize,
    field: StructuralFieldId,
) -> Option<Vec<crate::StructuralPathSegment>> {
    let (common, cases) = match shape {
        StructuralTypeShape::Sum { cases } => (&[][..], cases),
        StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
        _ => return None,
    };
    let selected = find_structural_case(cases, selected_case)?;
    let (declaration, is_payload) = common
        .iter()
        .map(|declaration| (declaration, false))
        .chain(
            selected
                .fields
                .iter()
                .map(|declaration| (declaration, true)),
        )
        .filter(|(declaration, _)| !declaration.relevance.is_erased())
        .nth(field_ordinal)?;
    if declaration.id != field {
        return None;
    }
    let mut path = Vec::new();
    if is_payload {
        path.push(crate::StructuralPathSegment::Case(selected_case));
    }
    path.push(crate::StructuralPathSegment::Field(declaration.identity));
    Some(path)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_fixed_array_contains_no_reference() {
        let referent = StructuralTypeId::new(1).unwrap();
        let reference = StructuralTypeId::new(2).unwrap();
        let element = StructuralTypeId::new(3).unwrap();
        let array = StructuralTypeId::new(4).unwrap();
        let types = vec![
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: reference,
                identity: crate::TerminalIdentityIndex(1),
                shape: StructuralTypeShape::Reference {
                    referent,
                    access: crate::StructuralAccess::Owned,
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: referent,
                identity: crate::TerminalIdentityIndex(2),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: element,
                identity: crate::TerminalIdentityIndex(3),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: crate::TerminalIdentityIndex(4),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(reference),
                        storage: None,
                    }],
                    storage: crate::StructuralRecordStorage::Packed,
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: array,
                identity: crate::TerminalIdentityIndex(5),
                shape: StructuralTypeShape::FixedArray { element, length: 0 },
            },
        ];
        assert!(structural_type_contains_reference(&types, reference));
        assert!(!structural_type_contains_reference(&types, array));
    }
}

/// The record-or-mixed field roster: record and mixed shapes carry the same
/// `fields` table, so consumers that only need the field set match the pair
/// with one spelling instead of duplicating both alternatives.
#[macro_export]
macro_rules! record_or_mixed_shape_pattern {
    ($fields:ident) => {
        $crate::StructuralTypeShape::Record {
            fields: $fields,
            ..
        } | $crate::StructuralTypeShape::Mixed {
            fields: $fields,
            ..
        }
    };
}

/// The record-or-fixed-array shape roster and the two byte-sequence view
/// carriers: verbatim or-pattern pairs consumers match with no bindings.
#[macro_export]
macro_rules! record_or_fixed_array_shape_pattern {
    () => {
        $crate::StructuralTypeShape::Record { .. } | $crate::StructuralTypeShape::FixedArray { .. }
    };
}

#[macro_export]
macro_rules! byte_sequence_view_carrier_pattern {
    () => {
        $crate::ByteSequenceCarrier::BorrowedView { .. }
            | $crate::ByteSequenceCarrier::BoundedView { .. }
    };
}

/// The sum-or-mixed shape roster consumers match with no bindings.
#[macro_export]
macro_rules! sum_or_mixed_shape_pattern {
    () => {
        $crate::StructuralTypeShape::Sum { .. } | $crate::StructuralTypeShape::Mixed { .. }
    };
}
