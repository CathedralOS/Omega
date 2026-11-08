/*
Physical ABI parameter homes, durable scalar homes, stack execution evidence,
structural store records, and target stack-frame layouts and protocols.

We represent the physical storage and memory write evidence attached to emitted
machine functions (`storage`) along with the target stack-frame geometry,
callee-save preservation plans, encoded prologue/epilogue protocols, and
applied frame records (`frames`, re-exported at `storage` and
`storage::frame_layout`).
*/

mod frames;

pub use frames::{
    CalleeSaveFrameSlot, FrameProtocolByteSpan, FrameUnwindPlan, FrameUnwindRestore,
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationIdentity, FunctionNonAuthoritativeCalleeSaveStorage,
    FunctionTargetFrameLayout, FunctionTargetFrameProtocolEncoding, LocalStorageFrameSlot,
    NonAuthoritativeCalleeSaveSlot, NonAuthoritativeCalleeSaveSlotId,
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy, OutgoingAbiFrameArea, ReturnAddressFrameCustody,
    StackProbePlan, TargetFrameLayoutIdentity, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, function_fragment_frame_application_identity,
    target_frame_layout_identity, target_frame_protocol_encoding_identity,
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
use semantic_vocabulary::{
    IeeeFloatValue, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType, StructuralFieldId,
    StructuralTypeId, ValueId,
};
use target_operations::MachineRegister;
use terminal_psi::StructuralPathSegment;

use crate::{
    InternalUnitScalarArgumentSourceRecord, ScalarCleanupPreservationEvidence,
    ScalarControlFlowEvidence,
};

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

impl UnitParameterRecord {
    pub fn matches_home(&self, home: &UnitParameterHomeRecord) -> bool {
        self.place == home.place
            && self.structural_type == home.structural_type
            && self.multiplicity == home.multiplicity
            && self.access == home.access
            && self.shape == home.shape
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterFunctionAbiRecord {
    pub call_plan: CallPlan,
    pub parameters: Vec<target_operations::ScalarAbiValue>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitScalarHomeRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitIntegerConstantRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: IntegerType,
    pub value: IntegerValue,
    pub operation_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAffineScalarRecordEstablishmentRecord {
    pub psi_operation: OperationId,
    pub result: terminal_psi::StructuralOperationResult,
    pub field: StructuralFieldId,
    pub value: IntegerValue,
    pub shape: ValueShape,
    pub operation_ordinal: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitStackEvidence {
    pub frame: Option<StackAdjustmentPair>, // Function-lifetime stack allocation and matching release
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>, // AArch64 incoming link register slot in function-lifetime frame
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitCallStackEvidence {
    pub outbound: Option<StackAdjustmentPair>, // Outgoing argument/shadow allocation and matching release around call
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarCallStackEvidence {
    pub outbound: Option<StackAdjustmentPair>,
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackAdjustmentPair {
    pub byte_size: u32,
    pub allocation_offset: usize,
    pub allocation_byte_count: usize,
    pub release_offset: usize,
    pub release_byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64ReturnLinkEvidence {
    pub frame_byte_offset: u32,
    pub store_offset: usize,
    pub load_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarStackEvidence {
    pub mutations: Vec<ScalarStackMutation>,
    pub control_flow: ScalarControlFlowEvidence,
    pub stack_alignment: u32,
    pub cleanup_preservation: Option<ScalarCleanupPreservationEvidence>, // ABI-result preservation around appended scalar-return cleanup suffix
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarStackMutation {
    pub offset: usize,
    pub byte_count: usize,
    pub kind: ScalarStackMutationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStackMutationKind {
    Allocate { byte_size: u32 },
    Release { byte_size: u32 },
    X86ReleasePreservingFlags { byte_size: u32 },
    X86Push,
    X86Pop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitStructuralScalarFieldStoreRecord {
    pub psi_operation: OperationId,
    pub destination: terminal_psi::StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub parameter_home_byte_offset: u32,
    pub parameter_home_indirect: bool,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitWriteOnlyPrimitiveStoreSourceRecord {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
        location: UnitScalarParameterLocationRecord,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
        definition_ordinal: usize,
    },
    IeeeFloatImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: IeeeFloatValue,
        definition_ordinal: usize,
    },
    Home(UnitScalarHomeRecord),
}

impl UnitWriteOnlyPrimitiveStoreSourceRecord {
    pub const fn defining_operation(&self) -> Option<OperationId> {
        match self {
            Self::Parameter { .. } => None,
            Self::IntegerImmediate {
                defining_operation, ..
            }
            | Self::BooleanImmediate {
                defining_operation, ..
            }
            | Self::IeeeFloatImmediate {
                defining_operation, ..
            } => Some(*defining_operation),
            Self::Home(home) => Some(home.defining_operation),
        }
    }

    pub const fn source_value(&self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. }
            | Self::IntegerImmediate { source_value, .. }
            | Self::BooleanImmediate { source_value, .. }
            | Self::IeeeFloatImmediate { source_value, .. } => *source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => *scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(*scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::IeeeFloatImmediate { value, .. } => ScalarType::IeeeFloat(value.format()),
            Self::Home(home) => home.scalar_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitWriteOnlyPrimitiveStoreRecord {
    pub psi_operation: OperationId,
    pub destination: terminal_psi::StructuralParameterDeclaration,
    pub destination_type: terminal_psi::StructuralTypeDeclaration,
    pub destination_placement: ValuePlacement,
    pub source: UnitWriteOnlyPrimitiveStoreSourceRecord,
    pub parameter_home_byte_offset: u32,
    pub parameter_home_indirect: bool,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarStructuralScalarFieldStoreRecord {
    pub psi_operation: OperationId,
    pub destination: terminal_psi::StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub immediate: target_operations::TargetScalarImmediate,
    pub return_operation: OperationId,
    pub return_source_value: ValueId,
    pub return_field: StructuralFieldId,
    pub return_field_byte_offset: u32,
    pub return_scalar_type: ScalarType,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
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
