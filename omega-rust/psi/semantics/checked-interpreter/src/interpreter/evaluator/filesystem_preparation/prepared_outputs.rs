//! Prepared transfer counts and byte and integer outputs.

use crate::interpreter::evaluator::filesystem_preparation::check_byte_len;
use crate::interpreter::evaluator::{Cell, EvalResult, Halt, Value, trap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedTransferCount {
    pub(crate) host: usize,
}

#[derive(Clone)]
pub(crate) enum PreparedByteOutput {
    Text {
        text: crate::value::TextBuffer,
        capacity: usize,
    },
    Array(Vec<Cell>),
}

impl PreparedByteOutput {
    pub(crate) fn capacity(&self) -> usize {
        match self {
            Self::Text { capacity, .. } => *capacity,
            Self::Array(cells) => cells.len(),
        }
    }

    pub(crate) fn require_capacity(&self, required: usize) -> EvalResult<()> {
        let capacity = self.capacity();
        if required > capacity {
            return trap(format!(
                "filesystem output requires {required} bytes but the prepared buffer holds {capacity}"
            ));
        }
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> EvalResult<Vec<u8>> {
        match self {
            Self::Text { text, .. } => Ok(text.borrow().to_vec()),
            Self::Array(cells) => cells.iter().map(prepared_byte).collect(),
        }
    }

    pub(crate) fn validate_contents(&self) -> EvalResult<()> {
        if let Self::Array(cells) = self {
            for cell in cells {
                prepared_byte(cell)?;
            }
        }
        Ok(())
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> EvalResult<()> {
        check_byte_len(bytes.len())?;
        self.require_capacity(bytes.len())?;
        match self {
            Self::Text { text, .. } => text.write_prefix(bytes).map_err(Halt::Trap)?,
            Self::Array(cells) => {
                for (slot, byte) in cells.iter().zip(bytes.iter()) {
                    *slot.borrow_mut() = Value::Int(i64::from(*byte));
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn prepared_byte(cell: &Cell) -> EvalResult<u8> {
    let value = cell.borrow();
    let Value::Int(raw) = *value else {
        return trap("filesystem byte-array operand contains a non-integer element");
    };
    u8::try_from(raw).map_err(|_| {
        Halt::Trap(format!(
            "filesystem byte-array operand element `{raw}` is outside u8 range"
        ))
    })
}

pub(crate) struct PreparedMutableByteInput {
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct PreparedI64Output {
    pub(crate) cell: Cell,
    pub(crate) initial: i64,
}

impl PreparedI64Output {
    pub(crate) fn validate_contents(&self) -> EvalResult<()> {
        if self.cell.borrow().as_int().is_none() {
            return trap("filesystem mutable scalar became non-integer");
        }
        Ok(())
    }

    pub(crate) fn write(&self, value: i64) -> EvalResult<()> {
        *self.cell.borrow_mut() = Value::Int(value);
        Ok(())
    }
}
