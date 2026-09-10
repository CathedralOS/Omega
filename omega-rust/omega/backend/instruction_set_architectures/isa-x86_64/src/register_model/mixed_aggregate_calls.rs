//! Mixed scalar input banks with explicit integer aggregate result fragments.
use super::*;

/// System V rows ordered by result fragment count (1–2), then mixed Unit-call key.
pub fn x86_64_system_v_mixed_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
    keys(1100..1212)
}

/// Microsoft rows retain mixed Unit-call positional slots and one rax result.
pub fn x86_64_microsoft_mixed_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
    keys(1220..1246)
}

fn keys(variants: std::ops::Range<u32>) -> Vec<RegisterConstraintKey> {
    variants
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
    for (input_keys, output_keys, fragment_count) in [
        (
            x86_64_system_v_mixed_unit_call_keys(),
            x86_64_system_v_mixed_aggregate_call_keys(),
            2,
        ),
        (
            x86_64_microsoft_mixed_unit_call_keys(),
            x86_64_microsoft_mixed_aggregate_call_keys(),
            1,
        ),
    ] {
        let mut output_keys = output_keys.into_iter();
        for fragments in 1..=fragment_count {
            for input_key in &input_keys {
                let Some(mut row) = constraints
                    .iter()
                    .find(|row| row.key == *input_key)
                    .cloned()
                else {
                    return;
                };
                let Some(key) = output_keys.next() else {
                    return;
                };
                row.key = key;
                for name in ["rax", "rdx"].into_iter().take(fragments) {
                    let Some(view) = model.model().view_named(name) else {
                        return;
                    };
                    row.operands.push(RegisterOperandConstraint {
                        operand: row.operands.len() as u16,
                        access: RegisterOperandAccess::Def,
                        class: view.class,
                        fixed_view: Some(view.id),
                        tied_to: None,
                        early_clobber: false,
                    });
                    // Definitions supply known result payloads; all other
                    // volatile units and call effects remain the Unit row's.
                    row.clobbers.retain(|unit| !view.write_units.contains(unit));
                }
                constraints.push(row);
            }
        }
    }
}

#[cfg(test)]
mod tests;
