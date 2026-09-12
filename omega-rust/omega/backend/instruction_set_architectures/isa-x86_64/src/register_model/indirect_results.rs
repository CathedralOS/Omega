//! Hidden aggregate destinations retain the native first-argument register as an input.
use super::*;

pub fn x86_64_indirect_aggregate_call_keys(microsoft: bool) -> Vec<RegisterConstraintKey> {
    let first = if microsoft { 2300 } else { 2200 };
    let count = if microsoft { 15 } else { 54 };
    (first..first + count)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    for microsoft in [false, true] {
        let Some(pointer) = model
            .model()
            .view_named(if microsoft { "rcx" } else { "rdi" })
        else {
            return;
        };
        let inputs = if microsoft {
            x86_64_microsoft_register_unit_call_keys()
        } else {
            x86_64_system_v_register_unit_call_keys()
        };
        let mixed = if microsoft {
            x86_64_microsoft_mixed_unit_call_keys()
        } else {
            x86_64_system_v_mixed_unit_call_keys()
        };
        let mut keys = x86_64_indirect_aggregate_call_keys(microsoft).into_iter();
        for input in inputs.into_iter().chain(mixed) {
            let Some(mut row) = constraints.iter().find(|row| row.key == input).cloned() else {
                return;
            };
            let Some(position) = row
                .operands
                .iter()
                .position(|operand| operand.fixed_view == Some(pointer.id))
            else {
                continue;
            };
            let Some(key) = keys.next() else {
                return;
            };
            row.key = key;
            // Semantic arguments retain their order; the separate hidden result input comes last.
            let hidden = row.operands.remove(position);
            row.operands.push(hidden);
            for (ordinal, operand) in row.operands.iter_mut().enumerate() {
                operand.operand = ordinal as u16;
            }
            constraints.push(row);
        }
    }
}
