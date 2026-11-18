/*
We retain strong selected-application custody for by-value opaque boundary
edges. Each `BoundaryOpaqueRepresentationApplication` records the exact
`selected_application_commitment` keyed by calling ABI, requirement identity,
and signature shape-graph root so independently compiled producer and consumer
artifacts compare the same application at every edge. Equal size, alignment, or
report fingerprints never establish agreement; canonicalization deduplicates
identical rows and fails closed when one edge coordinate carries two distinct
commitments.
*/

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundaryOpaqueRepresentationApplication {
    pub abi: semantic_vocabulary::CallingAbiIdentity,
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub shape_root: u16,
    pub application_report_fingerprint: u64,
    pub selected_application_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryOpaqueRepresentationApplications {
    rows: Vec<BoundaryOpaqueRepresentationApplication>,
}

impl BoundaryOpaqueRepresentationApplications {
    pub const EMPTY: Self = Self { rows: Vec::new() };

    pub fn new(
        mut rows: Vec<BoundaryOpaqueRepresentationApplication>,
    ) -> Result<Self, &'static str> {
        for row in &rows {
            row.abi.validate()?;
        }
        rows.sort();
        rows.dedup();
        for pair in rows.windows(2) {
            let [left, right] = pair else { unreachable!() };
            if left.abi == right.abi && left.shape_root == right.shape_root {
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
