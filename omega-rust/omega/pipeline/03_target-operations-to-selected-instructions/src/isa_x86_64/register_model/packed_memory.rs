//! Multi-instruction exact-byte memory transport declares all temporary writes.

use crate::register_model::{
    RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    ValidatedPhysicalRegisterModel,
};
pub const X86_64_LOAD_PACKED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 734,
};
pub const X86_64_STORE_PACKED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 735,
};
use crate::isa_x86_64::register_model::physical_model::GPR64;

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    let Some(flags) = model.model().view_named("rflags") else {
        return;
    };
    for (key, load) in [(X86_64_LOAD_PACKED, true), (X86_64_STORE_PACKED, false)] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: (0..3)
                .map(|operand| {
                    let writes = operand == 2 || (load && operand == 1);
                    RegisterOperandConstraint {
                        operand,
                        access: if writes {
                            RegisterOperandAccess::Def
                        } else {
                            RegisterOperandAccess::Use
                        },
                        class: GPR64,
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: writes,
                    }
                })
                .collect(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: flags.units.clone(),
        });
    }
}
