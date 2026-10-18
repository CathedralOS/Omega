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
    pub base: TargetReferenceOriginBase,
    pub path: Vec<terminal_psi::ReferenceOriginProjection>,
    pub extent: semantic_vocabulary::ReferenceOriginExtent,
}

/// Storage coordinates and captured pointer values are not interchangeable.
/// Replacing an incoming slot changes its contents, not the entry value retained
/// by an earlier copy. `path` on the origin projects from this base's payload.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetReferenceOriginBase {
    Storage {
        root: PlaceId,
    },
    IncomingContents {
        root: PlaceId,
        slot: Vec<StructuralPathSegment>,
    },
    /// A completed result establishes a descriptor independently of formal
    /// input storage. Its result place identifies the occurrence, not a
    /// physical container for the referent.
    Invocation {
        root: PlaceId,
        carrier_type: StructuralTypeId,
    },
    /// The admitted output establishes backing without identifying a caller
    /// storage root; it need not lie within an input but may alias one.
    /// Static invocation sites identify provenance, never allocation freshness
    /// or disjointness. The pointer is loaded from its actual carrier.
    Provider {
        boundary: semantic_vocabulary::BoundaryMachineId,
        output: u32,
        invocations: Vec<(semantic_vocabulary::MachineId, OperationId)>,
        referent: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    },
}

impl TargetReferenceOriginBase {
    /// A real storage/captured-slot root, never a stand-in for provider memory.
    pub fn root_place(&self) -> Option<PlaceId> {
        match self {
            Self::Storage { root } | Self::IncomingContents { root, .. } => Some(*root),
            Self::Invocation { .. } | Self::Provider { .. } => None,
        }
    }
}

impl TargetReferenceOrigin {
    /// A declaration-relative path for type interpretation only. An incoming
    /// slot contributes its declared referent type, not its current value.
    /// This path grants neither a reload address nor storage containment.
    pub fn type_projection(
        &self,
    ) -> std::borrow::Cow<'_, [terminal_psi::ReferenceOriginProjection]> {
        match &self.base {
            TargetReferenceOriginBase::Storage { .. }
            | TargetReferenceOriginBase::Invocation { .. }
            | TargetReferenceOriginBase::Provider { .. } => std::borrow::Cow::Borrowed(&self.path),
            TargetReferenceOriginBase::IncomingContents { slot, .. } => {
                let mut path = slot
                    .iter()
                    .cloned()
                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                    .collect::<Vec<_>>();
                path.push(terminal_psi::ReferenceOriginProjection::Exact(
                    StructuralPathSegment::Referent,
                ));
                path.extend_from_slice(&self.path);
                std::borrow::Cow::Owned(path)
            }
        }
    }

    /// Coarse memory-effect geometry, explicitly separate from pointer identity.
    /// Captured entry contents are indirect even when the slot has a fixed path;
    /// different slots supply no proof that their pointees are disjoint.
    pub fn storage_envelope(&self) -> Option<(PlaceId, Vec<StructuralPathSegment>, bool, bool)> {
        let root = self.base.root_place()?;
        if let TargetReferenceOriginBase::IncomingContents { slot, .. } = &self.base {
            return Some((root, slot.clone(), false, true));
        }
        let path = self
            .path
            .iter()
            .map_while(|projection| match projection {
                terminal_psi::ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                terminal_psi::ReferenceOriginProjection::Element => None,
            })
            .collect::<Vec<_>>();
        let exact = path.len() == self.path.len();
        let indirect = self
            .path
            .contains(&terminal_psi::ReferenceOriginProjection::Exact(
                StructuralPathSegment::Referent,
            ));
        Some((root, path, exact, indirect))
    }

    /// Only exact origins can name a stored descriptor resource. A family
    /// retains possible backing, not a usable load/store address.
    pub fn exact_path(&self) -> Option<Vec<StructuralPathSegment>> {
        if !matches!(self.base, TargetReferenceOriginBase::Storage { .. })
            || self.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
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
