//! Prepared logical handle inputs, outputs, retirements and plans.

use crate::interpreter::evaluator::FilesystemLogicalHandleKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedFilesystemLogicalHandleInput {
    pub(crate) operand_ordinal: u8,
    pub(crate) kind: FilesystemLogicalHandleKind,
    pub(crate) raw: i64,
    pub(crate) null_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilesystemLogicalHandleResultSuccess {
    NonNegative,
    NotMinusOne,
    Zero,
    NonZero,
}

impl FilesystemLogicalHandleResultSuccess {
    pub(crate) const fn accepts(self, result: i64) -> bool {
        match self {
            Self::NonNegative => result >= 0,
            Self::NotMinusOne => result != -1,
            Self::Zero => result == 0,
            Self::NonZero => result != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparedFilesystemLogicalHandleOutput {
    Created {
        kind: FilesystemLogicalHandleKind,
        success: FilesystemLogicalHandleResultSuccess,
    },
    Duplicated {
        source_operand_ordinal: u8,
        success: FilesystemLogicalHandleResultSuccess,
    },
    Borrowed {
        source_operand_ordinal: u8,
        success: FilesystemLogicalHandleResultSuccess,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilesystemLogicalHandleRetirementSuccess {
    Zero,
    NonZero,
}

impl FilesystemLogicalHandleRetirementSuccess {
    pub(crate) const fn accepts(self, result: i64) -> bool {
        match self {
            Self::Zero => result == 0,
            Self::NonZero => result != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedFilesystemLogicalHandleRetirement {
    pub(crate) operand_ordinal: u8,
    pub(crate) success: FilesystemLogicalHandleRetirementSuccess,
}

pub(crate) struct PreparedFilesystemLogicalHandlePlan {
    pub(crate) inputs: Vec<PreparedFilesystemLogicalHandleInput>,
    pub(crate) input_success: Option<FilesystemLogicalHandleResultSuccess>,
    pub(crate) output: Option<PreparedFilesystemLogicalHandleOutput>,
    pub(crate) retirement: Option<PreparedFilesystemLogicalHandleRetirement>,
}
