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
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
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
    RuntimeFrameIndexedBinaryWrite,
    RuntimeMachineStringWrite,
    RuntimePointeeStringWrite,
    RuntimeTextLineRead,
    RuntimeStorageCopy,
    RuntimeStorageCopyToRuntimeFrameIndexed,
    RuntimeStorageCopyToRuntimePointee,
    DispatchStateWrite,
    DispatchTerminate,
    DispatchCaseLeave,
    HostCallSequence,
    Return,
}
