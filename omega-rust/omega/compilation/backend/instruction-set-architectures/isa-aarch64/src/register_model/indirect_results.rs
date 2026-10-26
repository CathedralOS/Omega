//! Hidden aggregate destinations are call inputs in X8, not scalar result definitions.

use super::{aarch64_mixed_unit_call_keys, aarch64_register_unit_call_keys};
use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    RegisterOperandConstraint, ValidatedPhysicalRegisterModel, find_register_constraint,
};
pub fn aarch64_indirect_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 2300 } else { 2200 };
    (first..first + 81)
        .map(RegisterConstraintKey::call)
        .collect()
}

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    let Some(pointer) = model.model().view_named("x8") else {
        return;
    };
    for darwin in [false, true] {
        let inputs = aarch64_register_unit_call_keys(darwin);
        let mixed = aarch64_mixed_unit_call_keys(darwin);
        for (input, key) in inputs
            .into_iter()
            .chain(mixed)
            .zip(aarch64_indirect_aggregate_call_keys(darwin))
        {
            let Some(mut row) = find_register_constraint(constraints, input).cloned() else {
                return;
            };
            row.key = key;
            row.operands.push(RegisterOperandConstraint {
                operand: row.operands.len() as u16,
                access: RegisterOperandAccess::Use,
                class: pointer.class,
                fixed_view: Some(pointer.id),
                tied_to: None,
                early_clobber: false,
            });
            constraints.push(row);
        }
    }
}
