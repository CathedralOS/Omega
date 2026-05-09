use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;
use omega_target_program::StateGuardOperator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodePlan {
    pub target: NativeTarget,
    pub functions: Arena<MachineFunctionCode>,
    pub instructions: Arena<MachineInstruction>,
    pub bytes: Arena<u8>,
    pub byte_count: usize,
}

impl Default for MachineCodePlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
            bytes: Arena::new(),
            byte_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFunctionCode {
    pub symbol: String,
    pub offset: usize,
    pub byte_count: usize,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineFunctionCode {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            offset: 0,
            byte_count: 0,
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstruction {
    pub selected_instruction_index: u32,
    pub offset: usize,
    pub byte_width: usize,
    pub bytes: HandleSpan<u8>,
    pub kind: MachineInstructionKind,
}

impl Default for MachineInstruction {
    fn default() -> Self {
        Self {
            selected_instruction_index: 0,
            offset: 0,
            byte_width: 0,
            bytes: HandleSpan::empty(),
            kind: MachineInstructionKind::NoBytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineInstructionKind {
    NoBytes,
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
    RuntimeTextLiteralCompare {
        literal: String,
    },
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
    RuntimeTextLiteralWrite {
        literal: String,
    },
    RuntimeTextLiteralSegmentWrite {
        byte_offset: usize,
        literal: String,
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
    RuntimeTextStoredPlaceAppend {
        source_offset: usize,
        target_offset: usize,
    },
    RuntimeTextLiteralAppend {
        target_offset: usize,
        literal: String,
    },
    RuntimeMachineIntegerWrite {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    RuntimeMachineStringWrite {
        byte_offset: usize,
        byte_length: usize,
    },
    RuntimeTextLineRead {
        target_offset: usize,
        byte_capacity: usize,
        syscall_number: u32,
        syscall_number_register: u8,
        supervisor_call: u16,
    },
    RuntimeStorageCopy {
        source_offset: usize,
        target_offset: usize,
        byte_count: usize,
    },
    DispatchStateWrite {
        dispatch_index: u32,
    },
    DispatchTerminate {
        terminal_dispatch_index: u32,
    },
    DispatchCaseLeave,
    HostCallSequence {
        capability: String,
        operation: String,
    },
    Return,
}
