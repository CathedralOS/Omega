//! Checked accepted-template identities join every admission-claim machine
//! with the template identity checking already computed. Specialization can
//! clone a template under fresh concrete symbols, and an unspecialized
//! accepted template has no specialization row to recover the identity from,
//! so checking publishes one row per accepted template here. Consumers read
//! these rows, never by rerunning operational or service-reach inference to
//! hash canonical template bytes.

/// One accepted machine template's checked portable identity.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedAcceptedTemplateClassification {
    pub machine: symbols::SymbolHandle,
    /// The universal template identity when the machine declares type
    /// parameters; a parameterless machine carries no template to commit.
    pub identity: Option<CheckedAcceptedTemplateIdentity>,
}

/// The retained fingerprint and commitment pair for one generic template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedAcceptedTemplateIdentity {
    pub report_fingerprint: u64,
    pub commitment: typed_trees::typed_trees::MachineTemplateCommitment,
}

/// Frozen checked classifications keyed by the template machine symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedAcceptedTemplateIdentities {
    rows: Box<[CheckedAcceptedTemplateClassification]>,
}

impl CheckedAcceptedTemplateIdentities {
    pub fn with_rows(rows: Vec<CheckedAcceptedTemplateClassification>) -> Self {
        Self {
            rows: rows.into_boxed_slice(),
        }
    }

    pub fn for_machine(
        &self,
        machine: symbols::SymbolHandle,
    ) -> Option<&CheckedAcceptedTemplateClassification> {
        self.rows.iter().find(|row| row.machine == machine)
    }

    pub fn rows(&self) -> &[CheckedAcceptedTemplateClassification] {
        &self.rows
    }
}
