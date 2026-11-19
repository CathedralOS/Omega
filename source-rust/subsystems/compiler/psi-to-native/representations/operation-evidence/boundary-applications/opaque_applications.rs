/*
We retain strong selected-application custody for by-value opaque boundary
edges. Each `BoundaryOpaqueRepresentationApplication` records the exact
`selected_application_commitment` for one calling ABI and signature shape-graph root,
alongside its published requirement identity so independently compiled producer and consumer
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
        // The published row order also includes the requirement index. Two
        // conflicting entries for one ABI and shape root can sort apart.
        let mut edges = std::collections::BTreeSet::new();
        if rows
            .iter()
            .any(|row| !edges.insert((&row.abi, row.shape_root)))
        {
            return Err("boundary opaque custody carries two different applications at one edge");
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

#[cfg(test)]
mod tests {
    use super::{
        BoundaryOpaqueRepresentationApplication, BoundaryOpaqueRepresentationApplications,
    };

    #[test]
    fn conflicting_opaque_uses_cannot_hide_behind_another_requirement() {
        let row = |requirement, shape_root, digest| BoundaryOpaqueRepresentationApplication {
            abi: semantic_vocabulary::CallingAbiIdentity {
                requirement_identity: "service::send".to_owned(),
                declaring_application: "service".to_owned(),
                operands: Vec::new(),
            },
            requirement_identity: terminal_psi::TerminalIdentityIndex(requirement),
            shape_root,
            application_report_fingerprint: 17,
            selected_application_commitment: [digest; 32],
        };
        let first = row(1, 4, 1);
        let other = row(2, 8, 2);
        let accepted = BoundaryOpaqueRepresentationApplications::new(vec![
            other.clone(),
            first.clone(),
            first.clone(),
        ])
        .unwrap();
        assert_eq!(accepted.rows(), &[first.clone(), other.clone()]);
        assert_eq!(
            BoundaryOpaqueRepresentationApplications::new(vec![first, other, row(3, 4, 3)]),
            Err("boundary opaque custody carries two different applications at one edge"),
        );
    }
}
