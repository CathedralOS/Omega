/*
Physical ABI parameter homes, durable scalar homes, and target stack-frame
layouts and protocols.

We represent the physical storage evidence attached to emitted machine
functions (`storage`) along with the target stack-frame geometry,
callee-save preservation plans, encoded prologue/epilogue protocols, and
applied frame records (`frames`, re-exported at `storage` and
`storage::frame_layout`).
*/

#[path = "storage_frames.rs"]
mod frames;

pub use frames::{
    CalleeSaveFrameSlot, FrameProtocolByteSpan, FrameProtocolStepSpan, FrameUnwindPlan,
    FrameUnwindRestore, FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol,
    FunctionFragmentFrameApplication, FunctionNonAuthoritativeCalleeSaveStorage,
    FunctionTargetFrameLayout, FunctionTargetFrameProtocolEncoding, LocalStorageFrameSlot,
    NonAuthoritativeCalleeSaveSlot, NonAuthoritativeCalleeSaveSlotId,
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy, OutgoingAbiFrameArea, ReturnAddressFrameCustody,
    StackProbePlan, TargetFrameLayoutIdentity, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, target_frame_layout_identity,
    target_frame_protocol_encoding_identity,
};

pub mod frame_layout {
    pub use super::frames::{
        CalleeSaveFrameSlot, FrameUnwindPlan, FrameUnwindRestore,
        FunctionNonAuthoritativeCalleeSaveStorage, FunctionTargetFrameLayout,
        LocalStorageFrameSlot, NonAuthoritativeCalleeSaveSlot, NonAuthoritativeCalleeSaveSlotId,
        NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
        NonAuthoritativeCalleeSaveStoragePolicy, OutgoingAbiFrameArea, ReturnAddressFrameCustody,
        StackProbePlan, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
    };
}

use calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use semantic_vocabulary::{OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId};
use target_vocabulary::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitParameterHomeRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: terminal_psi::StructuralAccess,
    pub shape: ValueShape,
    pub source: ValuePlacement,
    pub location: StructuralSourceLocation,
    pub indirect: bool, // Valid only with exact ABI pointer placement; never denotes an invented stack home
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
pub struct ParameterFunctionAbiRecord {
    pub call_plan: CallPlan,
    pub parameters: Vec<target_vocabulary::ScalarAbiValue>,
    pub entry_register_spills: Vec<UnitEntryRegisterSpillRecord>, // Entry stores preserving incoming register values
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralSourceLocation {
    Stack {
        byte_offset: u32,
    },
    IncomingIndirectPointer {
        register: MachineRegister,
    },
    IncomingIndirectStackPointer {
        stack_byte_offset: u32, // Pointer's ABI offset, not payload copy or local home
        alignment: u16,
    },
    IncomingBorrowedPointer {
        location: calling_conventions::IndirectPointerLocation, // Original borrowed referent reached through incoming ABI pointer
    },
    /// An unchanged incoming pointer whose declared pointee has no by-value representation.
    IncomingOpaquePointer {
        location: calling_conventions::IndirectPointerLocation,
    },
}

impl StructuralSourceLocation {
    pub const fn stack_byte_offset(self) -> Option<u32> {
        match self {
            Self::Stack { byte_offset } => Some(byte_offset),
            Self::IncomingIndirectPointer { .. }
            | Self::IncomingIndirectStackPointer { .. }
            | Self::IncomingBorrowedPointer { .. }
            | Self::IncomingOpaquePointer { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitScalarHomeRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub byte_offset: u32,
}

#[cfg(test)]
mod tests {
    use super::{MachineRegister, StructuralSourceLocation};

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
