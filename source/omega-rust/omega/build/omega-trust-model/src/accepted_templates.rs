use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

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
    commitment: psi_typed_trees::typed_trees::MachineTemplateCommitment,
}

impl AcceptedTemplateIdentity {
    pub const fn report_fingerprint(self) -> u64 {
        self.report_fingerprint
    }

    pub const fn commitment(self) -> psi_typed_trees::typed_trees::MachineTemplateCommitment {
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
                    machine.supply_mode == psi_language_semantics::MachineSupplyMode::AdmissionClaim
                })
                .map(|machine| {
                    let report_fingerprint = psi_typed_trees_to_checked_trees::
                        generic_machine_template_report_fingerprint(
                            typed,
                            machine.symbol,
                        );
                    let commitment =
                        psi_typed_trees_to_checked_trees::generic_machine_template_commitment(
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_semantics::MachineSupplyMode;

    #[test]
    fn capture_retains_only_accepted_machine_templates() {
        let first = SymbolHandle::from_arena_index(1);
        let binder = SymbolHandle::from_arena_index(2);
        let checked = SymbolHandle::from_arena_index(3);
        let second = SymbolHandle::from_arena_index(4);
        let mut typed = TypedTrees::default();
        let mut generic_accepted = psi_typed_trees::machine::Machine {
            symbol: first,
            supply_mode: MachineSupplyMode::AdmissionClaim,
            ..Default::default()
        };
        typed.push_machine_type_parameter(
            &mut generic_accepted,
            psi_typed_trees::data::TypeParameter {
                symbol: binder,
                name: psi_typed_trees::name::Identifier::generated("T"),
                ..Default::default()
            },
        );
        typed.push_machine(generic_accepted);
        for (symbol, supply_mode) in [
            (checked, MachineSupplyMode::CheckedBody),
            (second, MachineSupplyMode::AdmissionClaim),
        ] {
            typed.push_machine(psi_typed_trees::machine::Machine {
                symbol,
                supply_mode,
                ..Default::default()
            });
        }

        let classifications = AcceptedTemplateClassifications::capture(&typed);
        let first_report_fingerprint =
            psi_typed_trees_to_checked_trees::generic_machine_template_report_fingerprint(
                &typed, first,
            )
            .expect("authored generic accepted machine must have a template report fingerprint");
        let first_commitment =
            psi_typed_trees_to_checked_trees::generic_machine_template_commitment(&typed, first)
                .expect("authored generic accepted machine must have a template commitment");

        assert_eq!(
            classifications.for_machine(first, "first"),
            Ok(Some(AcceptedTemplateIdentity {
                report_fingerprint: first_report_fingerprint,
                commitment: first_commitment,
            }))
        );
        assert_eq!(classifications.for_machine(second, "second"), Ok(None));
        assert!(classifications.for_machine(checked, "checked").is_err());
    }
}
