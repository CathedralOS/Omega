//! Fixed register Unit calls retain independent SysV banks and positional Microsoft slots.
use super::*;

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
