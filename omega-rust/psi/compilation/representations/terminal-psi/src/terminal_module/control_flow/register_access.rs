//! Observable scalar register events with exact operand roles.

use language_core::inline_assembly::{AsmRegisterOperation, AsmSystemRegister};
use semantic_vocabulary::ValueId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAccessEvent {
    Read {
        register: AsmSystemRegister,
    },
    Write {
        register: AsmSystemRegister,
        value: ValueId,
    },
}

impl RegisterAccessEvent {
    pub const fn operation(self) -> AsmRegisterOperation {
        match self {
            Self::Read { register } => AsmRegisterOperation::SystemRead(register),
            Self::Write { register, .. } => AsmRegisterOperation::SystemWrite(register),
        }
    }

    pub fn inputs(&self) -> &[ValueId] {
        match self {
            Self::Read { .. } => &[],
            Self::Write { value, .. } => std::slice::from_ref(value),
        }
    }

    pub fn map_operands(&mut self, mut map: impl FnMut(ValueId) -> ValueId) {
        if let Self::Write { value, .. } = self {
            *value = map(*value);
        }
    }
}
