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
    RuntimeFrameStringWrite,
    RuntimePointeeStringWrite,
    RuntimeFrameIndexedStringWrite,
    RuntimeMachineIndexedStringWrite,
    RuntimeStorageAddressToRuntimeFrameWrite,
    RuntimePointeeAddressToRuntimeFrameWrite,
    RuntimeFrameIndexedAddressToRuntimeFrameWrite,
    RuntimeFrameFixedIndexedAddressToRuntimeFrameWrite,
    RuntimeFrameBaseIndexedAddressToRuntimeFrameWrite,
    RuntimeTextLineRead,
    RuntimeStorageCopy,
    RuntimeStorageCopyToRuntimeFrameIndexed,
    RuntimeStorageCopyFromRuntimeFrameIndexed,
    RuntimeStorageCopyFromRuntimeFrameFixedIndexed,
    RuntimeStorageCopyFromRuntimeFrameFixedIndexedToRuntimePointee,
    RuntimeStorageCopyFromRuntimeMachineIndexed,
    RuntimeStorageCopyToRuntimePointee,
    RuntimeStorageCopyFromRuntimePointeeToRuntimeFrame,
    DispatchStateWrite,
    ReturnRegisterIntegerWrite,
    DispatchTerminate,
    DispatchCaseLeave,
    HostCallSequence,
    Return,
}
