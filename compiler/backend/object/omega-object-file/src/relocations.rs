use crate::ObjectSymbolHandle;
use omega_core::arena::{Arena, Handle};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlan {
    pub target: NativeTarget,
    pub record_set: RelocationRecordSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecordSet {
    pub records: Arena<RelocationRecord>,
}

impl Default for RelocationPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            record_set: RelocationRecordSet {
                records: Arena::new(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecord {
    pub function_symbol_handle: ObjectSymbolHandle,
    pub selected_instruction_index: u32,
    pub text_offset: usize,
    pub byte_width: usize,
    pub symbol_handle: ObjectSymbolHandle,
    pub kind: RelocationKind,
}

impl Default for RelocationRecord {
    fn default() -> Self {
        Self {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 0,
            text_offset: 0,
            byte_width: 0,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
    X86_64Absolute64,
    X86_64Relative32,
}
