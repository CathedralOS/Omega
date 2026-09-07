//! Semantic input declarations and their physical ABI homes.

use calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use semantic_vocabulary::{PlaceId, StructuralTypeId, ValueId};
use target_operations::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitParameterHomeRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: terminal_psi::StructuralAccess,
    pub shape: ValueShape,
    pub source: ValuePlacement,
    pub location: StructuralSourceLocation,
    /// Stack slots can contain direct bytes or a saved pointer. An incoming
    /// pointer location is valid only with `indirect == true` and the exact ABI
    /// source register; it never denotes an invented stack home.
    pub indirect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitParameterRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: terminal_psi::StructuralAccess,
    pub shape: ValueShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScalarFunctionAbiRecord {
    pub call_plan: CallPlan,
    pub parameters: Vec<target_operations::ScalarAbiValue>,
    /// Exact entry stores; ABI declarations above remain the original inputs.
    pub entry_register_spills: Vec<UnitEntryRegisterSpillRecord>,
}

/// An incoming register value preserved before structural staging and calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitEntryRegisterSpillRecord {
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub byte_offset: u32,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitScalarParameterLocationRecord {
    Register(MachineRegister),
    IncomingStack { byte_offset: u32 },
    FrameSpill { byte_offset: u32 },
}

/// Actual residence of a structural source. Incoming pointers do not imply
/// an unperformed spill or copy into a stack home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralSourceLocation {
    Stack { byte_offset: u32 },
    IncomingIndirectPointer { register: MachineRegister },
}

impl StructuralSourceLocation {
    pub const fn stack_byte_offset(self) -> Option<u32> {
        match self {
            Self::Stack { byte_offset } => Some(byte_offset),
            Self::IncomingIndirectPointer { .. } => None,
        }
    }
}
