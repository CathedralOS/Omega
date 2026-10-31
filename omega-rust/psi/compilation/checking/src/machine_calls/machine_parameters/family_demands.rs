//! Admit a portable application's static selections against every member.
//!
//! A family shares its carrier telescope, not a lexical member's callback
//! contract. Keep each authored demand intact and use ordinary callable and
//! nominal admission for it. Member inference uses positional copies of the
//! head's bindings; member-owned symbols never escape into the occurrence's
//! canonical specialization. Deferred effect checks retain every telescope.

use super::{
    CallSelection, FormedStaticMachineSelections, MachineStateIndex, TypeBinding,
    form_call_selection, machine_and_state,
};
use diagnostics::Diagnostic;

pub(super) fn form(
    program: &dyn crate::CheckingInput,
    index: &MachineStateIndex,
    call: CallSelection<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    occurrence: &mut FormedStaticMachineSelections,
) {
    let Some((head, _)) = machine_and_state(program, index, call.target_symbol) else {
        return;
    };
    let Some(family) = program
        .target_machine_families
        .iter()
        .find(|family| family.canonical == head.symbol)
    else {
        return;
    };
    let Some(specialization) = occurrence.rows.requirement_call_specializations.first() else {
        return;
    };
    let head_parameters = program.machine_type_parameters(head);
    for member in &family.members {
        if member.machine == head.symbol {
            continue;
        }
        let Some(machine) = program.machine_by_symbol(member.machine) else {
            diagnostics.push(Diagnostic::error(
                "target-family demand lost its retained declaration",
            ));
            return;
        };
        let member_parameters = program.machine_type_parameters(machine);
        let mut bindings = Vec::with_capacity(specialization.type_bindings.len());
        for binding in &specialization.type_bindings {
            let corresponding = head_parameters
                .iter()
                .position(|parameter| parameter.symbol == binding.parameter)
                .and_then(|position| member_parameters.get(position));
            let Some(parameter) = corresponding else {
                diagnostics.push(Diagnostic::error(
                    "target-family demand lost its generic binder correspondence",
                ));
                return;
            };
            bindings.push(TypeBinding {
                symbol: parameter.symbol,
                actual: binding.actual,
            });
        }
        let mut admitted = FormedStaticMachineSelections::default();
        form_call_selection(
            program,
            index,
            CallSelection {
                target_symbol: machine.symbol,
                ..call
            },
            &bindings,
            diagnostics,
            &mut admitted,
        );
        // Only the portable occurrence owns published selections. Each
        // member's requirements still own their independent effect checks;
        // discarding them would admit a callback outside a member's ceiling.
        occurrence.pending_effects.extend(admitted.pending_effects);
    }
}
