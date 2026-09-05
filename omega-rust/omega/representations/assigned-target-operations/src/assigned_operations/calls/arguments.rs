//! calls arguments in the assigned operations program.

use crate::AssignedScalarExpression;
use target_operations::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedCallArgument {
    pub scalar_type: semantic_vocabulary::ScalarType,
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
