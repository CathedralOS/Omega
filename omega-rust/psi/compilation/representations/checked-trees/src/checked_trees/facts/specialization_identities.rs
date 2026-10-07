//! Checked specialization commitments join the prepared instance symbol with
//! the selected machines' proved public contracts. Typed preparation retains
//! the template and argument roster, but cannot publish this conclusion before
//! checking has produced those contracts. Consumers read it here, never from a
//! checking-time rewrite of the typed program.

/// One prepared instance's checked portable identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedMachineSpecializationIdentity {
    pub instance: symbols::SymbolHandle,
    pub machine_argument_contract_commitments: Box<[[u8; 32]]>,
    pub commitment: typed_trees::typed_trees::MachineSpecializationCommitment,
}

/// Frozen checked conclusions keyed by the complete generational instance handle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedSpecializationIdentities {
    rows: Box<[CheckedMachineSpecializationIdentity]>,
}

impl CheckedSpecializationIdentities {
    pub fn with_rows(rows: Vec<CheckedMachineSpecializationIdentity>) -> Self {
        Self {
            rows: rows.into_boxed_slice(),
        }
    }

    pub fn for_instance(
        &self,
        instance: symbols::SymbolHandle,
    ) -> Option<&CheckedMachineSpecializationIdentity> {
        self.rows
            .iter()
            .find(|row| instance.is_valid() && row.instance == instance)
    }
}
