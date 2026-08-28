use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

/// Accepted-machine template identities captured before specialization can
/// clone a template under fresh concrete symbols.
#[derive(Clone, Default)]
pub struct AcceptedTemplateClassifications {
    rows: Vec<AcceptedTemplateClassification>,
}

#[derive(Clone)]
struct AcceptedTemplateClassification {
    machine: SymbolHandle,
    fingerprint: Option<u64>,
}

impl AcceptedTemplateClassifications {
    pub fn capture(typed: &TypedTrees) -> Self {
        Self {
            rows: typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
                })
                .map(|machine| AcceptedTemplateClassification {
                    machine: machine.symbol,
                    fingerprint:
                        psi_typed_trees_to_checked_trees::generic_machine_template_fingerprint(
                            typed,
                            machine.symbol,
                        ),
                })
                .collect(),
        }
    }

    pub fn for_machine(
        &self,
        machine: SymbolHandle,
        machine_name: &str,
    ) -> Result<Option<u64>, Diagnostic> {
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
        Ok(row.fingerprint)
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
            supply_mode: MachineSupplyMode::Accepted,
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
            (second, MachineSupplyMode::Accepted),
        ] {
            typed.push_machine(psi_typed_trees::machine::Machine {
                symbol,
                supply_mode,
                ..Default::default()
            });
        }

        let classifications = AcceptedTemplateClassifications::capture(&typed);
        let first_fingerprint =
            psi_typed_trees_to_checked_trees::generic_machine_template_fingerprint(&typed, first)
                .expect("authored generic accepted machine must have a template fingerprint");

        assert_eq!(
            classifications.for_machine(first, "first"),
            Ok(Some(first_fingerprint))
        );
        assert_eq!(classifications.for_machine(second, "second"), Ok(None));
        assert!(classifications.for_machine(checked, "checked").is_err());
    }
}
