//! Fixed register Unit-call rows with independently allocated integer and IEEE banks.
use super::*;

/// AAPCS64 rows ordered by GPR count (0–8), then IEEE register count (1–8).
pub fn aarch64_aapcs64_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
    keys(800..872)
}

/// Darwin rows ordered by GPR count (0–8), then IEEE register count (1–8).
pub fn aarch64_darwin_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
    keys(872..944)
}

fn keys(variants: std::ops::Range<u16>) -> Vec<RegisterConstraintKey> {
    variants
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: u32::from(variant),
        })
        .collect()
}

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    for (base_key, keys) in [
        (
            aarch64_aapcs64_register_unit_call_keys()[0],
            aarch64_aapcs64_mixed_unit_call_keys(),
        ),
        (
            aarch64_darwin_register_unit_call_keys()[0],
            aarch64_darwin_mixed_unit_call_keys(),
        ),
    ] {
        let Some(base) = constraints.iter().find(|row| row.key == base_key).cloned() else {
            continue;
        };
        let mut keys = keys.into_iter();
        for general_count in 0..=8 {
            for float_count in 1..=8 {
                let mut row = base.clone();
                let Some(key) = keys.next() else { return };
                row.key = key;
                let Some(operands) = (0..general_count)
                    .map(|position| format!("x{position}"))
                    .chain((0..float_count).map(|position| format!("d{position}")))
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
        }
    }
}
