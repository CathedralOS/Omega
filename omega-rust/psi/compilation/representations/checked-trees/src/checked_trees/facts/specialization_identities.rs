//! Checked specialization commitments join the prepared instance symbol with
//! the selected machines' proved public contracts. Typed preparation retains
//! the generic declaration and argument roster; neither an early template hash
//! nor an emitted instance's narrowed effect row supplies its universal contract.
//! Checking publishes both commitments after producing the public contract plans.
//! Consumers read them here, never from a
//! checking-time rewrite of the typed program.

/// One prepared instance's checked portable identity.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedMachineSpecializationIdentity {
    pub instance: symbols::SymbolHandle,
    /// Universal authored contract, never the selected instance's narrowed row.
    pub template_contract_commitment: typed_trees::typed_trees::MachineTemplateCommitment,
    pub machine_argument_contract_commitments: Box<[[u8; 32]]>,
    pub commitment: typed_trees::typed_trees::MachineSpecializationCommitment,
}

/// Frozen checked conclusions keyed by the complete generational instance handle.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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
