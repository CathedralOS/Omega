//! Structural type plans: unit structural types, cases, fields, domains,
//! parameters and path segments.

use language_core::BindingRelevance;
use language_semantics::{Multiplicity, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTrivialAffineStructuralLocalPlan {
    pub declaration_ordinal: u32,
    pub type_identity: String,
    /// Present only for a statically established fixed-array construction
    /// element. The root stays semantic metadata and never becomes an input.
    pub construction: Option<CheckedAffineConstructionElementPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedAffineConstructionElementPlan {
    pub root_type_identity: String,
    pub index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralResultPlan {
    pub type_identity: String,
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

/// One concrete target-neutral structural shape. Identities are normalized
/// semantic type identities rather than source-tree handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralTypePlan {
    pub identity: String,
    pub shape: CheckedUnitStructuralTypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitStructuralTypeShape {
    /// An owned reference carrier, distinct from the borrowed referent shape.
    Reference {
        referent_identity: String,
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
    },
    /// The owning indexed aggregate carrier: its extent is the declaration's
    /// own literal length, so the length is part of the type identity rather
    /// than a stored value. A runtime extent belongs to `BorrowedSliceView`,
    /// which borrows this storage instead of owning it.
    FixedArray {
        element_type_identity: String,
        length: u64,
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
    BorrowedSliceView { element_type_identity: String },
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
    pub identity: String,
    pub fields: Vec<CheckedUnitStructuralFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralFieldPlan {
    pub identity: String,
    pub relevance: BindingRelevance,
    pub field_type: CheckedUnitStructuralFieldType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitStructuralFieldType {
    Scalar(PrimitiveType),
    BoundedInteger(semantic_vocabulary::BoundedIntegerType),
    ByteSequence(CheckedByteSequenceCarrier),
    Structural {
        type_identity: String,
    },
    /// An authored dynamic boundary-trait field whose runtime carrier is
    /// eliminated only by an exact provider-installation specialization.
    /// The enclosing machine plan must retain one requirement row for every
    /// boundary call routed through this field.
    ProviderBacked {
        provider_type_identity: String,
    },
    /// An exact `Binding<R> in Bound` carrier authorized for erasure by the
    /// matching Fused selected-provider plan. Keeping this separate from the
    /// transitional bare-trait form makes receipt removal a rejecting
    /// corruption rather than a downgrade to legacy behavior.
    FusedServiceBacked {
        provider_type_identity: String,
        erasure: CheckedFusedServiceErasureReceipt,
    },
    /// An erased semantic field does not require an executable structural
    /// carrier. Its exact normalized type identity remains independently
    /// checkable in terminal Psi.
    Erased {
        type_identity: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFusedServiceErasureReceipt {
    pub requirement: SymbolHandle,
    pub provider_plan_digest: [u8; 32],
}

/// Exact compiler-owned authority for erasing one direct, owned
/// `Binding<R> in Bound` state parameter in a Fused build. The typed symbol
/// and normalized full carrier identity survive independently of the
/// structural parameter's dense position and erased base shape so a removed,
/// moved, or fabricated receipt rejects at the checked-to-Terminal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFusedServiceParameterReceipt {
    pub source_parameter: SymbolHandle,
    pub carrier_type_identity: String,
    pub requirement: SymbolHandle,
    pub provider_plan_digest: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedByteSequenceCarrier {
    BorrowedView,
    BoundedOwned { capacity: u64 },
}

/// One normalized structural qualification required by a retained parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralDomainPlan {
    pub domain: SemanticDomainId,
    pub identity: String,
    pub carrier_type_identity: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralParameterPlan {
    /// Position in the authored state signature. Structural argument lists use
    /// their own dense order and therefore never reinterpret this coordinate.
    pub position: u32,
    pub is_self: bool,
    pub type_identity: String,
    pub multiplicity: Multiplicity,
    pub access: CheckedStructuralAccess,
    /// Strictly ordered normalized domain identities.
    pub qualifications: Vec<SemanticDomainId>,
    pub projected_qualifications: Vec<CheckedStructuralPathQualification>,
    /// Present only for the first direct, owned affine
    /// `Binding<R> in Bound` parameter rung. `None` on a typed Service
    /// parameter is a rejecting custody downgrade, never legacy behavior.
    pub fused_service_erasure: Option<CheckedFusedServiceParameterReceipt>,
}

/// Source-handle-free structural path retained by checked terminal plans.
/// Cases deliberately have no variant in this vocabulary. A runtime index is
/// not a widened predicate or a trusted byte offset: `RuntimeIndex` names
/// where the checked selector's scalar value lives, and Terminal lowering
/// evaluates that value into the segment's `RuntimeIndex { index, obligation }`
/// spelling, whose `index < extent` bound the verifier re-proves at the
/// operation that carries the path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedUnitStructuralPathSegment {
    Field(String),
    FixedIndex(u64),
    /// Terminal, call-scoped window over initialized fixed byte-array backing.
    FixedByteRange {
        start: u64,
        end: u64,
    },
    /// A runtime-selected element of a fixed array. Fields and further
    /// indexes may follow it.
    RuntimeIndex(CheckedRuntimeIndex),
    Referent,
}

/// The checked scalar a `RuntimeIndex` segment selects with.
///
/// The segment carries no bound: the checked producer admitted the selector,
/// and Terminal re-proves `0 <= index < extent` from the facts that hold
/// before the operation (the caller's published requires for an entry
/// parameter, a dominating guard, a stored field's snapshot). Restating an
/// interval here would be a second, trusted copy of that evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedRuntimeIndex {
    /// The dense direct scalar parameter position of the calling state -- the
    /// coordinate `CheckedScalarExpression::Parameter` uses.
    Parameter { position: u32 },
    /// The owning assignment's evaluated `AssignmentIndex` scalar: the
    /// selector of its indexed target, evaluated before the stored value.
    AssignmentIndex,
}
