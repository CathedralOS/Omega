//! Structural type plans: unit structural types, cases, fields, domains,
//! parameters and path segments.

use language_core::BindingRelevance;
use language_semantics::{Multiplicity, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::PrimitiveType;

/// Checked-side name for the shared data-member identity; the handle
/// lives in typed-trees so facts and producers share one vocabulary.
pub use typed_trees::data::DataMemberIdentity as CheckedMemberIdentity;

/// A fixed array's declared extent as the shared judgment knows it: the
/// authored literal, or the retained recipe only a target closure fixes.
pub use typed_trees::constant_recipes::DeclaredArrayExtent as CheckedExtent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTrivialAffineStructuralLocalPlan {
    pub declaration_ordinal: u32,
    pub type_identity: TypeIdentityId,
    /// Present only for a statically established fixed-array construction
    /// element. The root stays semantic metadata and never becomes an input.
    pub construction: Option<CheckedAffineConstructionElementPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedAffineConstructionElementPlan {
    pub root_type_identity: TypeIdentityId,
    pub index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralResultPlan {
    pub type_identity: TypeIdentityId,
    pub multiplicity: Multiplicity,
    pub qualifications: Vec<SemanticDomainId>,
    pub projected_qualifications: Vec<CheckedStructuralPathQualification>,
}

/// A qualification belongs to its exact subplace, independently of the claim
/// identity or content theorem at that place. Rows use nonempty canonical paths
/// and sort by path followed by normalized domain identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CheckedStructuralPathQualification {
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub domain: SemanticDomainId,
}

/// One concrete target-neutral structural shape. Identities are interned
/// semantic type identity handles rather than source-tree handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralTypePlan {
    pub identity: TypeIdentityId,
    /// Exact source contents admit affine disposal without executing a hook,
    /// settling a linear claim, or disposing borrowed referent storage. This
    /// is a type capability, not proof that any particular place is live or
    /// that its loans have ended. Selected graph custody consumes this beside
    /// its owned, claim-free operation and qualification judgments; a missing
    /// final-exit drop does not erase this capability.
    pub no_code_affine_cleanup: bool,
    /// Checked authority to retire contained reference descriptors without
    /// observing old payloads or executing nominal cleanup. Exact live
    /// custody and child closure remain obligations of each replacement.
    pub descriptor_cleanup: bool,
    pub shape: CheckedUnitStructuralTypeShape,
    /// Declaration-owned scalar restriction; no membership or store evidence
    /// is created merely by retaining this schema.
    pub scalar_restriction: Option<CheckedUnitStructuralFieldType>,
}

/// Physical storage remains unresolved until the exact target layout is closed.
/// The typed application owns its checked alternatives; this row retains the
/// precise application and schema needed by source-free Terminal publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitRecordStorage {
    Packed,
    /// Logical placed interpretation; schema storage is an independent root.
    PlacedView {
        schema_type_identity: TypeIdentityId,
        storage: semantic_vocabulary::AuthoredRecordStorage,
    },
    PlacedAccessor(semantic_vocabulary::AuthoredRecordStorage),
    Authored(semantic_vocabulary::AuthoredRecordStorage),
    TargetLayout {
        application: SymbolHandle,
        schema_type_identity: TypeIdentityId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitStructuralTypeShape {
    /// A routed binding occurrence, independent of provider selection.
    /// This is not an empty record: construction requires establishment.
    /// Realization determines storage only after selecting its provider.
    BindingCarrier,
    /// An owned reference carrier, distinct from the borrowed referent shape.
    Reference {
        referent_identity: TypeIdentityId,
        access: CheckedStructuralAccess,
    },
    /// One whole primitive referent carried through structural custody. This
    /// is distinct from a by-value scalar parameter: the place remains the
    /// identity of an existing live value across an exclusive borrow.
    PrimitiveScalar(typed_trees::types::PrimitiveType),
    /// Immutable view over exact literal octets. This is semantic custody,
    /// not an assertion about a target pointer/length layout.
    ByteSequence(CheckedByteSequenceCarrier),
    /// Field order is declaration order; field identities are normalized
    /// declaration identities rather than source spellings alone.
    Record {
        fields: Vec<CheckedUnitStructuralFieldPlan>,
        storage: CheckedUnitRecordStorage,
    },
    /// The owning indexed aggregate carrier: its extent is the declaration's
    /// own literal length, so the length is part of the type identity rather
    /// than a stored value. A runtime extent belongs to `BorrowedSliceView`,
    /// which borrows this storage instead of owning it.
    FixedArray {
        element_type_identity: TypeIdentityId,
        length: CheckedExtent,
    },
    /// A borrowed `&[T]` view: the contiguous elements of storage some other
    /// value owns, carrying its own extent as a stored runtime length. Fixed
    /// arrays and vectors own storage and slices borrow it, and a slice's
    /// stored length is an ordinary runtime dependence measured by
    /// `Slice::Length` rather than a declared constant — so no length appears
    /// in this shape, and the view is a distinct type from any one backing
    /// extent. Borrow access stays on the parameter, argument or value plan
    /// that carries the view, exactly as it does for the borrowed
    /// `ByteSequence` carrier, and the pointer/length descriptor that realizes
    /// it stays a target-side implementation detail.
    BorrowedSliceView {
        element_type_identity: TypeIdentityId,
        access: Option<super::CheckedStructuralAccess>,
    },
    /// A closed pure sum. Each case owns its exact payload-field roster; an
    /// empty roster is the payload-less case form.
    Sum {
        cases: Vec<CheckedUnitStructuralCasePlan>,
    },
    /// A closed sum with common fields. Common-field and case declaration
    /// order are both semantic; payload fields remain owned by their case.
    Mixed {
        fields: Vec<CheckedUnitStructuralFieldPlan>,
        cases: Vec<CheckedUnitStructuralCasePlan>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralCasePlan {
    pub identity: CheckedMemberIdentity,
    pub fields: Vec<CheckedUnitStructuralFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralFieldPlan {
    pub identity: CheckedMemberIdentity,
    pub relevance: BindingRelevance,
    pub field_type: CheckedUnitStructuralFieldType,
    pub storage: Option<semantic_vocabulary::AuthoredFieldStorage>,
    /// The carrier types this field's domain qualifications are registered
    /// against when the field type itself names no structural type (a
    /// qualified byte view such as `&[u8] in Utf8`). A value constructed into
    /// the field carries those memberships, so a catalog that keeps the
    /// owner keeps these carriers and their domain theories with it.
    pub qualification_carriers: Vec<TypeIdentityId>,
}

impl CheckedUnitStructuralFieldPlan {
    /// Every structural type a catalog must keep alongside this field's
    /// owner: the field's own structural type and its qualification carriers.
    pub fn retained_type_identities(&self) -> impl Iterator<Item = TypeIdentityId> + '_ {
        self.field_type
            .structural_type_identity()
            .into_iter()
            .chain(self.qualification_carriers.iter().copied())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitStructuralFieldType {
    Scalar(PrimitiveType),
    /// Atomic storage retains its access contract separately from its carrier.
    Atomic(PrimitiveType),
    BoundedInteger(semantic_vocabulary::BoundedIntegerType),
    /// Complete scalar restriction over formal parameter zero. This coordinate
    /// names the field value, never an enclosing machine parameter.
    RefinedScalar {
        primitive: PrimitiveType,
        predicate: crate::CheckedBooleanExpression,
    },
    ByteSequence(CheckedByteSequenceCarrier),
    PredicateByteSequence {
        carrier: CheckedByteSequenceCarrier,
        predicates: language_semantics::byte_predicates::ByteSequencePredicateSet,
    },
    Structural {
        type_identity: TypeIdentityId,
    },
    /// An authored dynamic boundary-trait field whose runtime carrier is
    /// eliminated only by an exact provider-installation specialization.
    /// The enclosing machine plan must retain one requirement row for every
    /// boundary call routed through this field.
    ProviderBacked {
        provider_type_identity: TypeIdentityId,
    },
    /// An exact logical Binding carrier. The receipt identifies its declared
    /// requirement; it does not assert erasure or grant establishment.
    BindingBacked {
        provider_type_identity: TypeIdentityId,
        receipt: CheckedBindingFieldReceipt,
    },
    /// An erased semantic field does not require an executable structural
    /// carrier. Its exact normalized type identity remains independently
    /// checkable in terminal Psi.
    Erased {
        type_identity: TypeIdentityId,
    },
}

impl CheckedUnitStructuralFieldType {
    /// The structural type identity behind a nested-record or binding
    /// field, the two carriers that keep a whole type inside a record.
    pub fn structural_type_identity(&self) -> Option<TypeIdentityId> {
        match self {
            Self::Structural { type_identity }
            | Self::BindingBacked {
                provider_type_identity: type_identity,
                ..
            } => Some(*type_identity),
            _ => None,
        }
    }

    pub fn byte_sequence_carrier(&self) -> Option<CheckedByteSequenceCarrier> {
        match self {
            Self::ByteSequence(carrier) | Self::PredicateByteSequence { carrier, .. } => {
                Some(*carrier)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedBindingFieldReceipt {
    pub requirement: SymbolHandle,
}

/// Exact source occurrence of a Binding parameter. The full carrier identity
/// and requirement survive independently of its structural position. Selection
/// and storage are realization decisions, not facts supplied by this receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBindingParameterReceipt {
    pub source_parameter: SymbolHandle,
    pub carrier_type_identity: TypeIdentityId,
    pub requirement: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedByteSequenceCarrier {
    /// A `&'r [u8]` or `&'r mut [u8]` descriptor. `access` binds on a record
    /// field, which is what a byte store's authority is read from, and is
    /// absent on a standalone view type, whose identity peels the reference
    /// shell so that `&mut [u8]` and `&write [u8]` share it. Terminal's twin
    /// carries the same distinction.
    BorrowedView {
        access: Option<CheckedStructuralAccess>,
    },
    /// A `&'r [u8; N]` or `&'r mut [u8; N]` reference to fixed storage. The
    /// bound is the declaration's own literal length — like `BoundedOwned`
    /// the extent lives in the type, but the referent stays in the caller's
    /// frame exactly as `BorrowedView`, so the descriptor carries no runtime
    /// length at all.
    BoundedView {
        access: Option<CheckedStructuralAccess>,
        capacity: u64,
    },
    BoundedOwned {
        capacity: u64,
    },
}

impl From<language_core::ReferenceAccess> for CheckedStructuralAccess {
    /// An authored reference's access as the structural one. Every producer
    /// that turns a `&T` type reference into a structural plan needs this, and
    /// repeating the arms per producer is how a carrier ends up shared when
    /// the source said `&mut`.
    fn from(access: language_core::ReferenceAccess) -> Self {
        match access {
            language_core::ReferenceAccess::Shared => Self::SharedBorrow,
            language_core::ReferenceAccess::Mutable => Self::MutableBorrow,
            language_core::ReferenceAccess::WriteOnly => Self::WriteOnlyBorrow,
        }
    }
}

impl CheckedByteSequenceCarrier {
    /// Whether this is a borrowed view, whatever access it carries. A reader
    /// that only needs the descriptor shape asks this; comparing against a
    /// constructed carrier would answer no for the other access. The bounded
    /// view is deliberately excluded: it borrows fixed storage whose extent
    /// is declared, so descriptor consumers that read a stored length would
    /// find none.
    pub const fn is_borrowed_view(self) -> bool {
        matches!(self, Self::BorrowedView { .. })
    }

    /// Whether the carrier reads through borrowed storage at all — the
    /// access-bearing descriptors (`BorrowedView`, `BoundedView`). Owned
    /// carriers answer no.
    pub const fn is_borrowed(self) -> bool {
        matches!(self, Self::BorrowedView { .. } | Self::BoundedView { .. })
    }
}

/// [`CheckedByteSequenceCarrier::is_borrowed_view`] over an absent carrier.
pub fn is_borrowed_view(carrier: Option<CheckedByteSequenceCarrier>) -> bool {
    carrier.is_some_and(CheckedByteSequenceCarrier::is_borrowed_view)
}

impl CheckedByteSequenceCarrier {
    /// This carrier as a standalone view type's, which binds no access.
    pub const fn without_access(self) -> Self {
        match self {
            Self::BorrowedView { .. } => Self::BorrowedView { access: None },
            Self::BoundedView { capacity, .. } => Self::BoundedView {
                access: None,
                capacity,
            },
            other => other,
        }
    }
}

/// One normalized structural qualification required by a retained parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralDomainPlan {
    pub byte_predicate: Option<language_semantics::byte_predicates::ByteSequencePredicate>,
    pub domain: SemanticDomainId,
    /// The domain's canonical name spelling; the `domain` id is the identity.
    pub name: String,
    pub carrier_type_identity: TypeIdentityId,
    /// Authored `established by` routes copied from the domain declaration in
    /// authored order. Lowering normalizes each to its source-free identity.
    pub establishment_routes: Vec<language_semantics::DomainEstablishmentRoute>,
}

/// One exact structural-domain precondition on a boundary argument. The
/// argument index is dense over `structural_parameters`; no source expression
/// or contract-fact handle survives into this plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitStructuralDomainRequirementPlan {
    pub argument_index: u32,
    pub domain: SemanticDomainId,
}

/// Semantic authority retained by one structural carrier. Borrowed modes have
/// the same physical pointer ABI, but are deliberately distinct in checked and
/// Terminal identity so lowering cannot widen a non-observing loan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralAccess {
    Owned,
    SharedBorrow,
    MutableBorrow,
    WriteOnlyBorrow,
}

/// Shared or mutable borrow — accesses that read through a borrow.
#[macro_export]
macro_rules! checked_borrowed_access_pair_pattern {
    () => {
        $crate::checked_trees::CheckedStructuralAccess::SharedBorrow
            | $crate::checked_trees::CheckedStructuralAccess::MutableBorrow
    };
}
impl CheckedStructuralAccess {
    /// The loan kind a borrowed access grants; `None` for owned carriers, which
    /// move rather than loan.
    pub fn borrow_access_kind(self) -> Option<crate::checked_trees::borrow::BorrowAccessKind> {
        use crate::checked_trees::borrow::BorrowAccessKind;
        Some(match self {
            Self::SharedBorrow => BorrowAccessKind::Read,
            Self::MutableBorrow => BorrowAccessKind::Mutable,
            Self::WriteOnlyBorrow => BorrowAccessKind::WriteOnly,
            Self::Owned => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralParameterPlan {
    /// Position in the authored state signature. Structural argument lists use
    /// their own dense order and therefore never reinterpret this coordinate.
    pub position: u32,
    pub type_identity: TypeIdentityId,
    pub multiplicity: Multiplicity,
    pub access: CheckedStructuralAccess,
    /// The primitive a borrowed formal lends when its referent is one
    /// (`&u8`, `&mut i64`): the catalog row at `type_identity` is then a
    /// `PrimitiveScalar`, so a caller binds the loan to primitive storage
    /// instead of a structural place. `None` for every owned carrier and for
    /// a borrow of a structural referent.
    pub primitive_referent: Option<PrimitiveType>,
    /// Strictly ordered normalized domain identities.
    pub qualifications: Vec<SemanticDomainId>,
    pub projected_qualifications: Vec<CheckedStructuralPathQualification>,
    /// Present for a logical Binding parameter, including its borrow shell.
    /// Absence on a Binding is a custody error, not permission to erase it.
    pub binding_carrier: Option<CheckedBindingParameterReceipt>,
}

/// Source-handle-free structural path retained by checked terminal plans.
/// A case retains its exact declaration identity; lowering resolves it within
/// the current sum and receiving verification requires live membership at the
/// operation that uses the path. A runtime index is
/// not a widened predicate or a trusted byte offset: `RuntimeIndex` names
/// where the checked selector's scalar value lives, and Terminal lowering
/// evaluates that value into the segment's `RuntimeIndex { index, obligation }`
/// spelling, whose `index < extent` bound the verifier re-proves at the
/// operation that carries the path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckedUnitStructuralPathSegment {
    /// Source element qualification contract; never an executable projection.
    AllElements,
    Field(CheckedMemberIdentity),
    Case(CheckedMemberIdentity),
    FixedIndex(u64),
    /// Terminal, call-scoped window over initialized fixed byte-array backing.
    FixedByteRange {
        start: u64,
        end: u64,
    },
    /// A runtime-selected element of a fixed array. Fields and further
    /// indexes may follow it.
    RuntimeIndex(CheckedRuntimeIndex),
    /// This operand owns evaluation of the selector before its next sibling.
    /// The result becomes an ordinary runtime index with a bounds obligation;
    /// the computation handle supplies execution, never a proof of the bound.
    ComputedIndex(crate::CheckedScalarComputationHandle),
    /// An admitted interpretation of the selected original storage. Following
    /// fields use this declaration's namespace, not renamed backing fields.
    /// It is not a descriptor crossing, a loan, or a permission transition.
    /// Only the retained reference formation can issue this step; a type match
    /// or equal geometry cannot authorize it. Target-selected offsets stay out
    /// of this portable path.
    RepresentationView {
        type_identity: TypeIdentityId,
        storage: super::CheckedReferenceStorage,
    },
    Referent,
}

impl Default for CheckedUnitStructuralPathSegment {
    fn default() -> Self {
        // Arena slot zero has no declaration identity and is never a path.
        Self::Field(CheckedMemberIdentity::default())
    }
}

// Path ordering canonicalizes static claim/reference rosters and keys the
// checked open-window set. A computed selector has no numeric order: its full
// arena identity only distinguishes keys within this checked product. It is
// never serialized as a static claim path or used to prove selector ordering.
impl Ord for CheckedUnitStructuralPathSegment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use CheckedUnitStructuralPathSegment::*;
        fn kind(segment: &CheckedUnitStructuralPathSegment) -> u8 {
            match segment {
                Field(_) => 0,
                Case(_) => 1,
                FixedIndex(_) => 2,
                FixedByteRange { .. } => 3,
                RuntimeIndex(_) => 4,
                Referent => 5,
                ComputedIndex(_) => 6,
                RepresentationView { .. } => 7,
                AllElements => 8,
            }
        }
        match (self, other) {
            (Field(left), Field(right)) | (Case(left), Case(right)) => left.cmp(right),
            (FixedIndex(left), FixedIndex(right)) => left.cmp(right),
            (
                FixedByteRange {
                    start: left_start,
                    end: left_end,
                },
                FixedByteRange {
                    start: right_start,
                    end: right_end,
                },
            ) => (left_start, left_end).cmp(&(right_start, right_end)),
            (RuntimeIndex(left), RuntimeIndex(right)) => left.cmp(right),
            (ComputedIndex(left), ComputedIndex(right)) => (left.arena_index(), left.generation())
                .cmp(&(right.arena_index(), right.generation())),
            (
                RepresentationView {
                    type_identity: left_type,
                    storage: left_storage,
                },
                RepresentationView {
                    type_identity: right_type,
                    storage: right_storage,
                },
            ) => (left_type, left_storage).cmp(&(right_type, right_storage)),
            _ => kind(self).cmp(&kind(other)),
        }
    }
}

impl PartialOrd for CheckedUnitStructuralPathSegment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub use facts::CheckedRuntimeIndex;

/// Whether `shape` is a nominal record whose fields all erase at plan time:
/// every retained field is an ordinary no-cleanup primitive scalar. Bounded
/// nominal records carrying any structural, view, sum, or byte-sequence field
/// keep their own cleanup plan instead.
pub fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
    match shape {
        CheckedUnitStructuralTypeShape::Record { fields, .. } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    &field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(
                        PrimitiveType::Bool
                            | PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Addr
                    )
                )
        }),
        CheckedUnitStructuralTypeShape::BindingCarrier
        | CheckedUnitStructuralTypeShape::Reference { .. }
        | CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
        | CheckedUnitStructuralTypeShape::ByteSequence(_)
        | CheckedUnitStructuralTypeShape::FixedArray { .. }
        | CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
        | CheckedUnitStructuralTypeShape::Sum { .. }
        | CheckedUnitStructuralTypeShape::Mixed { .. } => false,
    }
}

/// No-cleanup leaf classification: structural containers, bounded integers,
/// refined scalars, bounded-owned byte sequences, and every primitive scalar
/// are partial-affine leaves — a partially moved value reconstructs the
/// untouched complement from these without executable cleanup.
pub fn is_partial_affine_field_type(field_type: &CheckedUnitStructuralFieldType) -> bool {
    matches!(
        field_type,
        CheckedUnitStructuralFieldType::Structural { .. }
            | CheckedUnitStructuralFieldType::BoundedInteger(_)
            | CheckedUnitStructuralFieldType::RefinedScalar { .. }
            | CheckedUnitStructuralFieldType::ByteSequence(
                CheckedByteSequenceCarrier::BoundedOwned { .. }
            )
            | CheckedUnitStructuralFieldType::PredicateByteSequence {
                carrier: CheckedByteSequenceCarrier::BoundedOwned { .. },
                ..
            }
            | CheckedUnitStructuralFieldType::Scalar(
                PrimitiveType::Bool
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Addr
                    | PrimitiveType::F32
                    | PrimitiveType::F64
            )
    )
}

/// The type plan carrying `identity` on a plan roster.
pub fn checked_structural_type_by_identity<'plans>(
    plans: &'plans [CheckedUnitStructuralTypePlan],
    identity: TypeIdentityId,
) -> Option<&'plans CheckedUnitStructuralTypePlan> {
    plans.iter().find(|plan| plan.identity == identity)
}
