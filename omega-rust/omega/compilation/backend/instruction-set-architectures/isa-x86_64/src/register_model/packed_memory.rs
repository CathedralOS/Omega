//! Multi-instruction exact-byte memory transport declares all temporary writes.

use register_model::{
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, ValidatedPhysicalRegisterModel,
};
pub const X86_64_LOAD_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(734);
pub const X86_64_STORE_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(735);
use crate::register_model::physical_model::GPR64;

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
                    let row = super::allocatable_operand(
                        operand,
                        if writes {
                            RegisterOperandAccess::Def
                        } else {
                            RegisterOperandAccess::Use
                        },
                        GPR64,
                    );
                    if writes {
                        super::early_clobber(row)
                    } else {
                        row
                    }
                })
                .collect(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: flags.units.clone(),
        });
    }
}
