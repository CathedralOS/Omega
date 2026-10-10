//! Accepted template identities and their classifications.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Accepted-machine template identities captured before specialization can
/// clone a template under fresh concrete symbols.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcceptedTemplateClassifications {
    rows: Vec<AcceptedTemplateClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcceptedTemplateClassification {
    machine: SymbolHandle,
    identity: Option<AcceptedTemplateIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedTemplateIdentity {
    report_fingerprint: u64,
    commitment: typed_trees::typed_trees::MachineTemplateCommitment,
}

impl AcceptedTemplateIdentity {
    pub const fn report_fingerprint(self) -> u64 {
        self.report_fingerprint
    }

    pub const fn commitment(self) -> typed_trees::typed_trees::MachineTemplateCommitment {
        self.commitment
    }
}

impl AcceptedTemplateClassifications {
    /// Read the identities checking published per accepted template. Checking
    /// already committed the same canonical bytes on its specialization rows;
    /// no operational or service-reach inference replays here.
    pub fn capture(checked: &checked_trees::CheckedTrees) -> Self {
        Self {
            rows: checked
                .facts
                .accepted_template_identities
                .rows()
                .iter()
                .map(|row| AcceptedTemplateClassification {
                    machine: row.machine,
                    identity: row.identity.map(|identity| AcceptedTemplateIdentity {
                        report_fingerprint: identity.report_fingerprint,
                        commitment: identity.commitment,
                    }),
                })
                .collect(),
        }
    }

    pub fn for_machine(
        &self,
        machine: SymbolHandle,
        machine_name: &str,
    ) -> Result<Option<AcceptedTemplateIdentity>, Diagnostic> {
        let mut matches = self.rows.iter().filter(|row| row.machine == machine);
        let row = matches.next().ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no pre-lowering template classification"
            ))
        })?;
        if matches.next().is_some() {
            return Err(Diagnostic::error(format!(
                "accepted machine `{machine_name}` has duplicate pre-lowering template classifications"
            )));
        }
        Ok(row.identity)
    }
}
