use omega_target_operations::MachineRegister;
use psi_core::{IntegerType, ValueId};

use crate::{AssignedBooleanExpression, AssignedIntegerExpression};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedCallArgument {
    pub scalar_type: psi_core::ScalarType,
    /// Concrete ABI home populated after all sibling arguments have been
    /// evaluated. Outgoing stack offsets are relative to the call plan's ABI
    /// argument area and therefore already include any shadow/home space.
    pub destination: AssignedCallDestination,
    /// Stable frame slot holding the fully evaluated argument until every
    /// sibling argument is ready for simultaneous ABI placement.
    pub spill_byte_offset: u32,
    pub expression: AssignedScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedCallDestination {
    Register(MachineRegister),
    OutgoingStack { byte_offset: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedScalarExpression {
    Boolean(AssignedBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: AssignedIntegerExpression,
    },
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
