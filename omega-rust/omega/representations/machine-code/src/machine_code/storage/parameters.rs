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
    /// pointer placement; it never denotes an invented stack home.
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

/// Incoming scalar identities accompany the complete ABI plan. Structural
/// parameter homes are recorded separately; a non-Unit result stays in the plan
/// and is realized by the retained function graph, not an invented scalar value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterFunctionAbiRecord {
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

/// Structural source residence or incoming ABI origin. Physical replay retains
/// subsequent pointer transport; an incoming location implies no referent copy
/// or invented activation-local home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralSourceLocation {
    Stack {
        byte_offset: u32,
    },
    IncomingIndirectPointer {
        register: MachineRegister,
    },
    /// Owned value backing reached through an incoming ABI stack pointer slot.
    /// This is the pointer's ABI offset, not the payload copy's offset or a local home.
    IncomingIndirectStackPointer {
        stack_byte_offset: u32,
        alignment: u16,
    },
    /// Original borrowed referent reached through the incoming ABI pointer.
    /// A stack location contains pointer bits, not a copied referent or local home.
    IncomingBorrowedPointer {
        location: calling_conventions::IndirectPointerLocation,
    },
}

impl StructuralSourceLocation {
    pub const fn stack_byte_offset(self) -> Option<u32> {
        match self {
            Self::Stack { byte_offset } => Some(byte_offset),
            Self::IncomingIndirectPointer { .. }
            | Self::IncomingIndirectStackPointer { .. }
            | Self::IncomingBorrowedPointer { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_stack_residence_supplies_a_stack_home_offset() {
        assert_eq!(
            StructuralSourceLocation::Stack { byte_offset: 32 }.stack_byte_offset(),
            Some(32)
        );
        for source in [
            StructuralSourceLocation::IncomingIndirectPointer {
                register: MachineRegister::X86Rcx,
            },
            StructuralSourceLocation::IncomingIndirectStackPointer {
                stack_byte_offset: 32,
                alignment: 8,
            },
            StructuralSourceLocation::IncomingBorrowedPointer {
                location: calling_conventions::IndirectPointerLocation::Register(
                    MachineRegister::Aarch64X(0),
                ),
            },
            StructuralSourceLocation::IncomingBorrowedPointer {
                location: calling_conventions::IndirectPointerLocation::Stack {
                    stack_byte_offset: 32,
                    alignment: 8,
                },
            },
        ] {
            assert_eq!(source.stack_byte_offset(), None, "{source:?}");
        }
    }
}
