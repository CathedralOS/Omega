use omega_calling_conventions::HostOperationKey;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;
use omega_target_operations::StateGuardOperator;
use std::sync::Arc;

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
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineInstructionKind {
    NoOp,
    DispatchLoopEnter {
        entry_dispatch_index: u32,
    },
    DispatchCaseEnter {
        dispatch_index: u32,
    },
    DispatchGuardCompareStatic {
        operator: StateGuardOperator,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
    },
    RuntimeTextLiteralCompare,
    RuntimeTextStorageCompare {
        source_offset: usize,
        operator: StateGuardOperator,
    },
    RuntimeStorageCompare {
        left_offset: usize,
        right_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    RuntimeStorageValueCompare {
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        operator: StateGuardOperator,
    },
    RuntimeTextLiteralWrite,
    RuntimeTextLiteralSegmentWrite {
        byte_offset: usize,
    },
    RuntimeTextStoredSuffixAppend {
        buffer_offset: usize,
        source_offset: usize,
        target_offset: usize,
        length_delta: usize,
    },
    RuntimeTextBufferMaterialize {
        target_offset: usize,
    },
    RuntimeTextBufferMaterializeToRuntimePointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    RuntimeTextBufferMaterializeToRuntimeFrameIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    RuntimeTextStoredPlaceAppend {
        source_offset: usize,
        target_offset: usize,
    },
    RuntimeTextStoredPlaceAppendToRuntimePointee {
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    RuntimeTextStoredPlaceAppendToRuntimeFrameIndexed {
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    RuntimeTextLiteralAppend {
        target_offset: usize,
    },
    RuntimeTextLiteralAppendToRuntimePointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    RuntimeTextLiteralAppendToRuntimeFrameIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    RuntimeMachineIntegerWrite {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    RuntimePointeeIntegerWrite {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    RuntimeStorageBinaryWrite {
        target_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    RuntimePointeeBinaryWrite {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    RuntimeFrameIndexedIntegerWrite {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    RuntimeFrameIndexedBinaryWrite {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    RuntimeMachineStringWrite {
        byte_offset: usize,
        byte_length: usize,
    },
    RuntimePointeeStringWrite {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_length: usize,
    },
    RuntimeTextLineRead {
        target_offset: usize,
        byte_capacity: usize,
    },
    RuntimeStorageCopy {
        source_offset: usize,
        target_offset: usize,
        byte_count: usize,
    },
    RuntimeStorageCopyToRuntimeFrameIndexed {
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_count: usize,
    },
    RuntimeStorageCopyToRuntimePointee {
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_count: usize,
    },
    DispatchStateWrite {
        dispatch_index: u32,
    },
    DispatchTerminate,
    DispatchCaseLeave,
    HostCallSequence {
        operation_key: HostOperationKey,
    },
    Return,
}
