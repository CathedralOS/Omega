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
/// `root` names the common referent root, and `root_paths` names the possible
/// referent paths under it. A whole-root singleton contains one empty path.
/// This roster proves custody, not which address an activation returns. Native
/// consumers load that address from the reference carrier's exact result home;
/// they cannot choose an arbitrary candidate to reconstruct its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetReferenceResult {
    pub path: Vec<StructuralPathSegment>,
    pub root: PlaceId,
    /// Nonempty, strictly ordered alternatives for one permission carrier.
    pub root_paths: Vec<Vec<StructuralPathSegment>>,
}

impl TargetReferenceResult {
    /// Static address reconstruction requires exactly one possible root path.
    pub fn sole_root_path(&self) -> Option<&[StructuralPathSegment]> {
        let [path] = self.root_paths.as_slice() else {
            return None;
        };
        Some(path)
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
