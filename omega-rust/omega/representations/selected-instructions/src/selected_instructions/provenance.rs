//! Semantic operations, values, edges, proof obligations and fuel retained by an instruction.
use optimization_unit::{FuelSettlement, PsiProvenance};
use semantic_vocabulary::{EdgeId, ObligationId, OperationId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedInstructionProvenance {
    pub operations: Vec<OperationId>,
    pub values: Vec<ValueId>,
    pub edges: Vec<EdgeId>,
    pub obligations: Vec<ObligationId>,
    pub fuel: Vec<FuelSettlement>,
}

impl SelectedInstructionProvenance {
    /// Logical fuel may settle only against the operations and edges this
    /// instruction already claims as source custody, and each site settles
    /// at most once for a nonzero unit count. A settlement naming a site the
    /// instruction never claims is foreign work, a repeated site double
    /// charges it, and a zero-unit settlement understates the charge.
    pub fn fuel_settles_only_claimed_sites(&self) -> bool {
        let mut settled = std::collections::BTreeSet::new();
        self.fuel.iter().all(|settlement| {
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
        EdgeId, FuelSettlement, OperationId, PsiProvenance, SelectedInstructionProvenance,
    };

    #[test]
    fn fuel_custody_binds_sites_uniqueness_and_units() {
        let operation = OperationId::new(2).expect("operation id");
        let edge = EdgeId::new(3).expect("edge id");
        let claimed = SelectedInstructionProvenance {
            operations: vec![operation],
            edges: vec![edge],
            fuel: vec![
                FuelSettlement {
                    site: PsiProvenance::Operation(operation),
                    units: 7,
                },
                FuelSettlement {
                    site: PsiProvenance::Edge(edge),
                    units: 11,
                },
            ],
            ..Default::default()
        };
        assert!(claimed.fuel_settles_only_claimed_sites());
        assert!(SelectedInstructionProvenance::default().fuel_settles_only_claimed_sites());
        // Claiming custody without settling it is legal: edge transport and
        // continuation instructions retain the source site while the owning
        // row charges the units.
        assert!(
            SelectedInstructionProvenance {
                edges: vec![edge],
                ..Default::default()
            }
            .fuel_settles_only_claimed_sites()
        );

        // A settlement against an operation the row never claims.
        let mut forged = claimed.clone();
        forged.fuel.push(FuelSettlement {
            site: PsiProvenance::Operation(OperationId::new(41).expect("operation id")),
            units: 1,
        });
        assert!(!forged.fuel_settles_only_claimed_sites());

        // A settlement against an edge the row never claims.
        let mut forged = claimed.clone();
        forged.fuel.push(FuelSettlement {
            site: PsiProvenance::Edge(EdgeId::new(43).expect("edge id")),
            units: 1,
        });
        assert!(!forged.fuel_settles_only_claimed_sites());

        // A claimed site may not settle twice.
        let mut forged = claimed.clone();
        forged.fuel.push(FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        });
        assert!(!forged.fuel_settles_only_claimed_sites());

        // No free ride: a claimed site still owes a nonzero charge.
        let mut forged = claimed.clone();
        forged.fuel[1].units = 0;
        assert!(!forged.fuel_settles_only_claimed_sites());

        // A settlement may not borrow a claimed id under the other site kind:
        // an operation id is not edge custody.
        assert!(
            !SelectedInstructionProvenance {
                operations: vec![operation],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(2).expect("edge id")),
                    units: 1,
                }],
                ..Default::default()
            }
            .fuel_settles_only_claimed_sites()
        );
    }
}
