//! Accepted template identities and their classifications.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

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
    pub fn capture(typed: &TypedTrees) -> Self {
        Self {
            rows: typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                })
                .map(|machine| {
                    let report_fingerprint =
                        typed_trees_to_checked_trees::generic_machine_template_report_fingerprint(
                            typed,
                            machine.symbol,
                        );
                    let commitment =
                        typed_trees_to_checked_trees::generic_machine_template_commitment(
                            typed,
                            machine.symbol,
                        );
                    let identity = report_fingerprint.zip(commitment).map(
                        |(report_fingerprint, commitment)| AcceptedTemplateIdentity {
                            report_fingerprint,
                            commitment,
                        },
                    );
                    AcceptedTemplateClassification {
                        machine: machine.symbol,
                        identity,
                    }
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
