use crate::{BlockId, OperationId, TargetIdentity, TypeId, ValueId};
pub use omega_target_contract::{InstructionExit, InstructionInput, InstructionSemantics, InstructionValue, InstructionWrite};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyRegister {
    pub name: String,
    pub ty: TypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssemblyOperand {
    Register(AssemblyRegister),
    Value(ValueId),
    LocalBlock(BlockId),
}

/// All initial values are evaluated before any working register is written.
/// Each execution of this operation establishes a fresh region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyEntry {
    pub target: TargetIdentity,
    pub bindings: Vec<(AssemblyRegister, ValueId)>,
    pub clobbers: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyInstruction {
    pub entry: OperationId,
    pub target: TargetIdentity,
    pub mnemonic: String,
    pub operands: Vec<AssemblyOperand>,
    pub implicit_registers: Vec<AssemblyRegister>,
    /// The selected executable state transition. A conditional exit produces
    /// its pre-instruction Boolean condition as this operation's scalar result.
    pub semantics: Arc<InstructionSemantics>,
}
