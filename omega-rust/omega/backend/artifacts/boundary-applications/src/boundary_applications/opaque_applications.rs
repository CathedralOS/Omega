//! Strong selected-application custody for opaque by-value boundary edges.
//!
//! A `BoundaryOpaqueRepresentationApplication` is the canonical, source-free
//! record of one opaque representation actually used by value while a boundary
//! signature was materialized. It carries the strong
//! `selected_application_commitment` (the closed-conformance plus
//! lifecycle/copy/origin derivation, not a size or fingerprint) keyed by the
//! requirement edge and the signature-graph shape coordinate, so independently
//! compiled producer and consumer artifacts compare the same application at
//! the same edge. Equal size, alignment, or the compact report fingerprint
//! never establish agreement; the commitment is the compare key.

/// One by-value boundary edge's exact selected opaque application.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundaryOpaqueRepresentationApplication {
    /// Canonical requirement identity of the boundary requirement whose
    /// signature materialized this use (the package-qualified edge name, never
    /// an arena handle).
    pub requirement_identity: String,
    /// Shape-graph root where the selected carrier entered the materialized
    /// boundary signature. Two occurrences of the same opaque declaration in
    /// one signature keep distinct coordinates.
    pub shape_root: u16,
    /// Compact compatibility/report coordinate retained beside the strong
    /// commitment; it is never an agreement input.
    pub application_report_fingerprint: u64,
    /// The strong selected-application commitment this edge exchanged.
    pub selected_application_commitment: [u8; 32],
}

/// The complete custody set one artifact retains for its by-value opaque
/// boundary edges, canonically ordered for stable replay.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryOpaqueRepresentationApplications {
    rows: Vec<BoundaryOpaqueRepresentationApplication>,
}

impl BoundaryOpaqueRepresentationApplications {
    /// The canonical empty custody: an artifact with no by-value opaque
    /// boundary edges retains this exact set rather than a `None` claim.
    pub const EMPTY: Self = Self { rows: Vec::new() };

    /// Canonicalize the row set: identical rows from paired semantic/physical
    /// realizations deduplicate, while one edge coordinate carrying two
    /// different commitments is producer/consumer drift and fails closed.
    pub fn new(
        mut rows: Vec<BoundaryOpaqueRepresentationApplication>,
    ) -> Result<Self, &'static str> {
        rows.sort();
        rows.dedup();
        for pair in rows.windows(2) {
            let [left, right] = pair else { unreachable!() };
            if left.requirement_identity == right.requirement_identity
                && left.shape_root == right.shape_root
            {
                return Err(
                    "boundary opaque custody carries two different applications at one edge",
                );
            }
        }
        Ok(Self { rows })
    }

    pub fn rows(&self) -> &[BoundaryOpaqueRepresentationApplication] {
        &self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
