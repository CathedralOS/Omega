//! Semantic operations, values, edges, proof obligations and work retained by an instruction.
use optimization_unit::{PsiProvenance, WorkSettlement};
use semantic_vocabulary::{EdgeId, ObligationId, OperationId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedInstructionProvenance {
    pub operations: Vec<OperationId>,
    pub values: Vec<ValueId>,
    pub edges: Vec<EdgeId>,
    pub obligations: Vec<ObligationId>,
    pub work: Vec<WorkSettlement>,
}

impl SelectedInstructionProvenance {
    /// Logical work may settle only against the operations and edges this
    /// instruction already claims as source custody, and each site settles
    /// at most once for a nonzero unit count. A settlement naming a site the
    /// instruction never claims is foreign work, a repeated site double
    /// charges it, and a zero-unit settlement understates the charge.
    pub fn work_settles_only_claimed_sites(&self) -> bool {
        let mut settled = std::collections::BTreeSet::new();
        self.work.iter().all(|settlement| {
            settlement.units != 0
                && settled.insert(settlement.site)
                && match settlement.site {
                    PsiProvenance::Operation(operation) => self.operations.contains(&operation),
                    PsiProvenance::Edge(edge) => self.edges.contains(&edge),
                }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EdgeId, OperationId, PsiProvenance, SelectedInstructionProvenance, WorkSettlement,
    };

    #[test]
    fn work_custody_binds_sites_uniqueness_and_units() {
        let operation = OperationId::new(2).expect("operation id");
        let edge = EdgeId::new(3).expect("edge id");
        let claimed = SelectedInstructionProvenance {
            operations: vec![operation],
            edges: vec![edge],
            work: vec![
                WorkSettlement {
                    site: PsiProvenance::Operation(operation),
                    units: 7,
                },
                WorkSettlement {
                    site: PsiProvenance::Edge(edge),
                    units: 11,
                },
            ],
            ..Default::default()
        };
        assert!(claimed.work_settles_only_claimed_sites());
        assert!(SelectedInstructionProvenance::default().work_settles_only_claimed_sites());
        // Claiming custody without settling it is legal: edge transport and
        // continuation instructions retain the source site while the owning
        // row charges the units.
        assert!(
            SelectedInstructionProvenance {
                edges: vec![edge],
                ..Default::default()
            }
            .work_settles_only_claimed_sites()
        );

        // A settlement against an operation the row never claims.
        let mut forged = claimed.clone();
        forged.work.push(WorkSettlement {
            site: PsiProvenance::Operation(OperationId::new(41).expect("operation id")),
            units: 1,
        });
        assert!(!forged.work_settles_only_claimed_sites());

        // A settlement against an edge the row never claims.
        let mut forged = claimed.clone();
        forged.work.push(WorkSettlement {
            site: PsiProvenance::Edge(EdgeId::new(43).expect("edge id")),
            units: 1,
        });
        assert!(!forged.work_settles_only_claimed_sites());

        // A claimed site may not settle twice.
        let mut forged = claimed.clone();
        forged.work.push(WorkSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        });
        assert!(!forged.work_settles_only_claimed_sites());

        // No free ride: a claimed site still owes a nonzero charge.
        let mut forged = claimed.clone();
        forged.work[1].units = 0;
        assert!(!forged.work_settles_only_claimed_sites());

        // A settlement may not borrow a claimed id under the other site kind:
        // an operation id is not edge custody.
        assert!(
            !SelectedInstructionProvenance {
                operations: vec![operation],
                work: vec![WorkSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(2).expect("edge id")),
                    units: 1,
                }],
                ..Default::default()
            }
            .work_settles_only_claimed_sites()
        );
    }
}
