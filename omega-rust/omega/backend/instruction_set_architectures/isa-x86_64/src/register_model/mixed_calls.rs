//! Fixed register Unit calls retain independent SysV banks and positional Microsoft slots.

use super::{x86_64_microsoft_register_unit_call_keys, x86_64_system_v_register_unit_call_keys};
use register_model::{
    RegisterConstraintFamily, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, ValidatedPhysicalRegisterModel,
};
/// System V rows ordered by GPR count (0–6), then IEEE register count (1–8).
pub fn x86_64_system_v_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
    keys(800..856)
}

/// Microsoft rows ordered by occupied slot count (1–4), then nonzero IEEE slot mask.
pub fn x86_64_microsoft_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
    keys(900..926)
}

fn keys(variants: std::ops::Range<u16>) -> Vec<RegisterConstraintKey> {
    variants
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: u32::from(variant),
        })
        .collect()
}

fn append_row(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
    base: &RegisterInstructionConstraint,
    key: RegisterConstraintKey,
    names: impl Iterator<Item = String>,
) {
    let mut row = base.clone();
    row.key = key;
    let Some(operands) = names
        .enumerate()
        .map(|(operand, name)| {
            let view = model.model().view_named(&name)?;
            Some(RegisterOperandConstraint {
                operand: operand as u16,
                access: RegisterOperandAccess::Use,
                class: view.class,
                fixed_view: Some(view.id),
                tied_to: None,
                early_clobber: false,
            })
        })
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    row.operands = operands;
    constraints.push(row);
}

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    let Some(system_v) = constraints
        .iter()
        .find(|row| row.key == x86_64_system_v_register_unit_call_keys()[0])
        .cloned()
    else {
        return;
    };
    let mut keys = x86_64_system_v_mixed_unit_call_keys().into_iter();
    for general_count in 0..=6 {
        for float_count in 1..=8 {
            let Some(key) = keys.next() else { return };
            append_row(
                constraints,
                model,
                &system_v,
                key,
                ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                    .into_iter()
                    .take(general_count)
                    .map(str::to_owned)
                    .chain((0..float_count).map(|position| format!("xmm{position}"))),
            );
        }
    }
    let Some(microsoft) = constraints
        .iter()
        .find(|row| row.key == x86_64_microsoft_register_unit_call_keys()[0])
        .cloned()
    else {
        return;
    };
    let mut keys = x86_64_microsoft_mixed_unit_call_keys().into_iter();
    for arity in 1..=4 {
        for float_mask in 1..(1_u16 << arity) {
            let Some(key) = keys.next() else { return };
            append_row(
                constraints,
                model,
                &microsoft,
                key,
                ["rcx", "rdx", "r8", "r9"]
                    .into_iter()
                    .take(arity)
                    .enumerate()
                    .filter(|(position, _)| float_mask & (1 << position) == 0)
                    .map(|(_, name)| name.to_owned())
                    .chain(
                        (0..arity)
                            .filter(|position| float_mask & (1 << position) != 0)
                            .map(|position| format!("xmm{position}")),
                    ),
            );
        }
    }
}

/// Reuse the canonical input banks without retaining source-signature permutations.
/// After the plain rows, one family per (input bank, integer-bank position)
/// carries a private callback slot: that parameter's register is pinned by the
/// materialization bytes inside the call encoding, so it is an implicit unit
/// use rather than an operand.
pub(super) fn append_normalized_foreign_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    let Some(integer_class) = model.model().view_named("rax").map(|view| view.class) else {
        return;
    };
    for (inputs, keys) in [
        (
            x86_64_system_v_register_unit_call_keys()
                .into_iter()
                .chain(x86_64_system_v_mixed_unit_call_keys()),
            super::x86_64_system_v_normalized_foreign_call_keys(),
        ),
        (
            x86_64_microsoft_register_unit_call_keys()
                .into_iter()
                .chain(x86_64_microsoft_mixed_unit_call_keys()),
            super::x86_64_microsoft_normalized_foreign_call_keys(),
        ),
    ] {
        let mut keys = keys.into_iter();
        for input in inputs.clone() {
            let Some(base) = constraints.iter().find(|row| row.key == input).cloned() else {
                return;
            };
            for result_name in [None, Some("rax"), Some("xmm0")] {
                let Some(key) = keys.next() else { return };
                let mut row = base.clone();
                row.key = key;
                append_result_operand(&mut row, model, result_name);
                constraints.push(row);
            }
        }
        for input in inputs {
            let Some(base) = constraints.iter().find(|row| row.key == input).cloned() else {
                return;
            };
            for position in 0..base.operands.len() {
                if base.operands[position].access != RegisterOperandAccess::Use {
                    continue;
                }
                let Some(callback_view) = base.operands[position]
                    .fixed_view
                    .and_then(|view_id| model.model().views.iter().find(|view| view.id == view_id))
                else {
                    return;
                };
                if callback_view.class != integer_class {
                    continue;
                }
                for result_name in [None, Some("rax"), Some("xmm0")] {
                    let Some(key) = keys.next() else { return };
                    let mut row = base.clone();
                    row.key = key;
                    row.operands.remove(position);
                    for (operand, slot) in row.operands.iter_mut().enumerate() {
                        slot.operand = operand as u16;
                    }
                    row.implicit_uses
                        .extend(callback_view.units.iter().copied());
                    row.implicit_uses.sort_unstable();
                    row.implicit_uses.dedup();
                    append_result_operand(&mut row, model, result_name);
                    constraints.push(row);
                }
            }
        }
    }
}

fn append_result_operand(
    row: &mut RegisterInstructionConstraint,
    model: &ValidatedPhysicalRegisterModel,
    result_name: Option<&str>,
) {
    if let Some(result_name) = result_name {
        let Some(result) = model.model().view_named(result_name) else {
            return;
        };
        row.operands.push(RegisterOperandConstraint {
            operand: row.operands.len() as u16,
            access: RegisterOperandAccess::Def,
            class: result.class,
            fixed_view: Some(result.id),
            tied_to: None,
            early_clobber: false,
        });
        // The explicit result defines its full architectural write
        // footprint. Every other caller-save unit stays clobbered.
        row.clobbers
            .retain(|unit| !result.write_units.contains(unit));
    }
}
