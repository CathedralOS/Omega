// Instruction operands are identical to the target layer's; the assigned layer
// adds nothing here. Share the one canonical definition (its InstructionOperandLike
// impl lives on the shared type in omega-target-operations) instead of re-declaring
// the struct/enum + two `From` conversions + the accessor impl.
pub use omega_target_operations::{
    TargetInstructionOperand as AssignedInstructionOperand,
    TargetInstructionOperandKind as AssignedInstructionOperandKind,
};

pub type InstructionOperand = AssignedInstructionOperand;
pub type InstructionOperandKind = AssignedInstructionOperandKind;
