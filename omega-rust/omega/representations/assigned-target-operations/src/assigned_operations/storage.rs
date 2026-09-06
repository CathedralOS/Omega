//! storage in the assigned operations program.

use semantic_vocabulary::ValueId;
use target_operations::MachineRegister;

pub mod scalar_call;
pub use scalar_call::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionFrame {
    /// Aligned bytes reserved before evaluating the expression.
    pub byte_size: u32,
    /// Incoming ABI registers copied into stable frame homes before any
    /// expression scratch register can overwrite them.
    pub register_spills: Vec<EntryRegisterSpill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRegisterSpill {
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedScalarLocation {
    Register(MachineRegister),
    /// Stable storage reserved by the assignment stage in the current frame.
    FrameSpill {
        byte_offset: u32,
    },
    /// Byte offset in the ABI's incoming stack-argument area. Machine emission
    /// accounts only for the assigned frame and return-address bias.
    IncomingStack {
        byte_offset: u32,
    },
}
