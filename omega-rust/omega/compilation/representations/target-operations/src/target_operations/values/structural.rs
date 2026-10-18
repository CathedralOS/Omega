//! Structural places, projected arguments, and field replacement facts.

use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId};
use terminal_psi::StructuralPathSegment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralParameter {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: terminal_psi::StructuralAccess,
    /// Exact authored path qualification roster. Target lowering transports
    /// this authority; it must never infer rows from layout or carrier shape.
    pub projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralArgument {
    pub place: PlaceId,
    pub access: terminal_psi::StructuralAccess,
    /// Exact source-relative semantic projection retained through native and
    /// installed-artifact custody.
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    /// Checked byte offset within source storage, or within the referent when
    /// `path` crosses a Reference. In that case `source` still names the
    /// carrier: its typed prefix selects the descriptor to load, and this
    /// offset describes only the suffix after that load. Loan origins are
    /// authority, not substitute physical addresses.
    pub source_byte_offset: u32,
    /// Root array extent for indexed projection. For a field-only or whole
    /// mutable fixed-u8-array presentation, this instead names the projected
    /// array extent: `shape` describes the view descriptor while `source`
    /// still identifies original backing, not a descriptor allocation.
    pub fixed_array_length: Option<u64>,
    /// Array element byte stride, paired with `fixed_array_length` (one for
    /// fixed-u8-array presentation). Metadata alone grants no indexing authority.
    pub element_stride: Option<u32>,
    /// One resolved `RuntimeIndex` traversal of `path` per row, in path order
    /// — resolved like `StructuralLeafCopy::indices`: the operand scales by
    /// `stride` before joining `source_byte_offset`. Empty unless `path`
    /// carries runtime segments.
    pub indices: Vec<crate::TargetStructuralRuntimeIndex>,
    pub source: TargetStructuralArgumentSource,
    pub destination: ValuePlacement,
}

/// One reference leaf inside a structural call result, resolved through
/// compile-time custody. `path` names the leaf within the result carrier,
/// `origins` names every possible referent root and path, including independently
/// rooted inputs. A whole-root origin has an empty path.
/// This roster proves custody, not which address an activation returns. Native
/// consumers load that address from the reference carrier's exact result home;
/// they cannot choose an arbitrary candidate to reconstruct its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetReferenceResult {
    pub path: Vec<StructuralPathSegment>,
    /// Nonempty, strictly ordered alternatives for one permission carrier.
    pub origins: Vec<TargetReferenceOrigin>,
}

/// Possible storage conservatively retained by a reference descriptor.
/// The root is logical custody, never a choice of runtime backing address.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetReferenceOrigin {
    /// Actual supplying storage or invocation result, never a synthetic input.
    pub root: PlaceId,
    pub kind: TargetReferenceOriginKind,
    pub path: Vec<terminal_psi::ReferenceOriginProjection>,
    pub extent: semantic_vocabulary::ReferenceOriginExtent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetReferenceOriginKind {
    Storage,
    Invocation { carrier_type: StructuralTypeId },
}

impl TargetReferenceOrigin {
    /// Only exact origins can name a stored descriptor resource. A family
    /// retains possible backing, not a usable load/store address.
    pub fn exact_path(&self) -> Option<Vec<StructuralPathSegment>> {
        if self.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
            || self.kind != TargetReferenceOriginKind::Storage
        {
            return None;
        }
        self.path
            .iter()
            .map(|projection| match projection {
                terminal_psi::ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                terminal_psi::ReferenceOriginProjection::Element => None,
            })
            .collect()
    }
}

/// An incoming value placement and an established local descriptor have
/// different storage origins, even when the callee receives the same pointer ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralArgumentSource {
    /// Complete owned aggregate established by this exact dominating producer.
    StructuralHome {
        psi_operation: OperationId,
    },
    /// Original activation-local primitive storage established by this operation.
    EstablishedPrimitiveLocal {
        psi_operation: semantic_vocabulary::OperationId,
    },
    Placement(ValuePlacement),
    /// Storage bound at this exact block entry, not a producer operation. The
    /// declaration distinguishes an owned aggregate home from a view descriptor.
    BlockParameter {
        block: semantic_vocabulary::BlockId,
        place: PlaceId,
    },
    /// Exact literal or subslice producer; its retained operation owns contents
    /// and bounds, and source validation independently establishes dominance.
    EstablishedByteView {
        psi_operation: OperationId,
    },
    /// Exact element-view establishment or subslice producer; its retained
    /// operation owns the descriptor derivation, and source validation
    /// independently establishes dominance.
    EstablishedElementView {
        psi_operation: OperationId,
    },
}

impl From<ValuePlacement> for TargetStructuralArgumentSource {
    fn from(placement: ValuePlacement) -> Self {
        Self::Placement(placement)
    }
}
