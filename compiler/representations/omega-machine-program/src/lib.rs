use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProgram {
    pub target: NativeTarget,
    pub functions: Arena<MachineFunction>,
    pub instructions: Arena<MachineInstruction>,
}

impl Default for MachineProgram {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0)
    }
}

impl MachineProgram {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFunction {
    pub source_key: StateKey,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineFunction {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineInstruction {
    pub selected_instruction_index: u32,
    pub kind: MachineInstructionKind,
}

impl Default for MachineInstruction {
    fn default() -> Self {
        Self {
            selected_instruction_index: 0,
            kind: MachineInstructionKind::NoOp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineInstructionKind {
    NoOp,
    DispatchLoopEnter,
    DispatchCaseEnter,
    DispatchGuardCompareStatic,
    RuntimeTextLiteralCompare,
    RuntimeTextStorageCompare,
    RuntimeStorageCompare,
    RuntimeStorageValueCompare,
    RuntimeTextLiteralWrite,
    RuntimeTextLiteralSegmentWrite,
    RuntimeTextStoredSuffixAppend,
    RuntimeTextBufferMaterialize,
    RuntimeTextBufferMaterializeToRuntimePointee,
    RuntimeTextBufferMaterializeToRuntimeFrameIndexed,
    RuntimeTextStoredPlaceAppend,
    RuntimeTextStoredPlaceAppendToRuntimePointee,
    RuntimeTextStoredPlaceAppendToRuntimeFrameIndexed,
    RuntimeTextLiteralAppend,
    RuntimeTextLiteralAppendToRuntimePointee,
    RuntimeTextLiteralAppendToRuntimeFrameIndexed,
    RuntimeMachineIntegerWrite,
    RuntimePointeeIntegerWrite,
    RuntimeStorageBinaryWrite,
    RuntimePointeeBinaryWrite,
    RuntimeFrameIndexedIntegerWrite,
    RuntimeFrameBaseIndexedIntegerWrite,
    RuntimeMachineIndexedIntegerWrite,
    RuntimeFrameIndexedBinaryWrite,
    RuntimeFrameBaseIndexedBinaryWrite,
    RuntimeMachineStringWrite,
    RuntimePointeeStringWrite,
    RuntimeFrameIndexedStringWrite,
    RuntimeMachineIndexedStringWrite,
    RuntimeStorageAddressToRuntimeFrameWrite,
    RuntimePointeeAddressToRuntimeFrameWrite,
    RuntimeFrameIndexedAddressToRuntimeFrameWrite,
    RuntimeFrameBaseIndexedAddressToRuntimeFrameWrite,
    RuntimeTextLineRead,
    RuntimeStorageCopy,
    RuntimeStorageCopyToRuntimeFrameIndexed,
    RuntimeStorageCopyFromRuntimeFrameIndexed,
    RuntimeStorageCopyFromRuntimeFrameFixedIndexed,
    RuntimeStorageCopyFromRuntimeMachineIndexed,
    RuntimeStorageCopyToRuntimePointee,
    DispatchStateWrite,
    ReturnRegisterIntegerWrite,
    DispatchTerminate,
    DispatchCaseLeave,
    HostCallSequence,
    Return,
}
