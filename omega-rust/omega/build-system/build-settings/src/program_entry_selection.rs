//! Build selects its required entry slot before source settlement. Native
//! planning later sees the corresponding accepted declaration and call plans.
use declaration_contracts::selection::RootBinding;
use diagnostics::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedProgramEntry<'config> {
    pub binding_ordinal: usize,
    pub machine_name: &'config str,
    pub machine_symbol: symbols::SymbolHandle, // Exact machine selected under the binding occurrence's lexical package.
    pub slot: target::ProgramEntrySlotDeclaration,
}

pub fn selected_program_entry_machine<'config>(
    root_bindings: &'config [RootBinding],
    selected_profile: Option<target::TargetProfile>,
) -> Result<Option<SelectedProgramEntry<'config>>, Vec<Diagnostic>> {
    let Some(selected_profile) = selected_profile else {
        return Ok(None);
    };
    if root_bindings.is_empty() {
        return Ok(None);
    }
    let mut diagnostics = Vec::new();
    let mut selected_bindings = Vec::new();
    for (ordinal, binding) in root_bindings.iter().enumerate() {
        let Some((profile, slot_name)) = binding.slot.rsplit_once("::") else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}` is not target-qualified; expected `target::ProgramEntry`",
                binding.slot
            )));
            continue;
        };
        let profile = match target::TargetProfile::from_root_slot_owner(profile) {
            Ok(profile) => profile,
            Err(_) => {
                diagnostics.push(Diagnostic::error(format!(
                    "root slot `{}` belongs to unknown target profile `{profile}`",
                    binding.slot
                )));
                continue;
            }
        };
        let Some(required_slot) = profile.required_root_slot(slot_name) else {
            diagnostics.push(Diagnostic::error(format!(
                "target profile `{}` declares no required root slot `{}`",
                profile.target_name(),
                binding.slot
            )));
            continue;
        };
        if profile != selected_profile {
            continue;
        }
        selected_bindings.push((required_slot, binding, ordinal));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut program_entries = Vec::new();
    for required_slot in selected_profile.required_root_slots() {
        let matches = selected_bindings
            .iter()
            .filter(|(selected, _, _)| selected == &required_slot)
            .collect::<Vec<_>>();
        let (binding, ordinal) = match matches.as_slice() {
            [(_, binding, ordinal)] => (*binding, *ordinal),
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has no bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has more than one bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
        };
        let Some(program_entry) = required_slot.program_entry() else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}::{}` uses a target-required schema that the ProgramEntry source-lowering path does not implement",
                required_slot.owner().root_slot_owner_name(),
                required_slot.slot_name()
            )));
            continue;
        };
        program_entries.push((program_entry, binding, ordinal));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    match program_entries.as_slice() {
        [(slot, binding, ordinal)] => Ok(Some(SelectedProgramEntry {
            binding_ordinal: *ordinal,
            machine_name: binding.implementation.as_str(),
            machine_symbol: binding.implementation_symbol,
            slot: *slot,
        })),
        [] => Err(vec![Diagnostic::error(format!(
            "selected target `{}` has no supported ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
        _ => Err(vec![Diagnostic::error(format!(
            "selected target `{}` declares more than one ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
    }
}
