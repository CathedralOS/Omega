//! Observed scalars, bytes and the mutable observation plans.

use crate::interpreter::evaluator::filesystem_preparation::prepared_outputs::{
    PreparedByteOutput, PreparedI64Output,
};
use crate::interpreter::evaluator::{EvalResult, trap};
use crate::{
    FilesystemByteOperand, FilesystemMutableByteOperand, FilesystemMutableByteOperandResolution,
    FilesystemMutableI64Operand, FilesystemMutableI64OperandResolution, FilesystemPathLikeOperand,
    FilesystemScalarOperand, FilesystemScalarOperandValue,
};

pub(crate) fn observed_scalar(
    operand_ordinal: u8,
    value: FilesystemScalarOperandValue,
) -> FilesystemScalarOperand {
    FilesystemScalarOperand {
        operand_ordinal,
        value,
    }
}

pub(crate) fn observed_bytes(operand_ordinal: u8, value: &[u8]) -> FilesystemByteOperand {
    FilesystemByteOperand {
        operand_ordinal,
        bytes: value.to_vec(),
    }
}

pub(crate) fn observed_path_like_bytes(
    operand_ordinal: u8,
    value: &[u8],
) -> FilesystemPathLikeOperand {
    FilesystemPathLikeOperand {
        operand_ordinal,
        bytes: value.to_vec(),
    }
}

pub(crate) struct PreparedFilesystemMutableByteObservation {
    pub(crate) operand_ordinal: u8,
    output: PreparedByteOutput,
    pre_bytes: Vec<u8>,
}

pub(crate) struct PreparedFilesystemMutableI64Observation {
    pub(crate) operand_ordinal: u8,
    output: PreparedI64Output,
    pre_value: i64,
}

pub(crate) struct PreparedFilesystemMutableObservationPlan {
    pub(crate) byte_operands: Vec<PreparedFilesystemMutableByteObservation>,
    pub(crate) i64_operands: Vec<PreparedFilesystemMutableI64Observation>,
}

impl PreparedFilesystemMutableObservationPlan {
    /// Resolution evidence describes the earlier argument-preparation instant,
    /// while this plan describes provider-visible state after all authored
    /// arguments have run. Cross-check only the stable operand role and carrier
    /// capacity: later argument evaluation may legitimately alter contents.
    pub(crate) fn matches_resolution_roles(
        &self,
        byte_resolutions: &[FilesystemMutableByteOperandResolution],
        i64_resolutions: &[FilesystemMutableI64OperandResolution],
    ) -> bool {
        byte_resolutions.len() == self.byte_operands.len()
            && byte_resolutions
                .iter()
                .zip(&self.byte_operands)
                .all(|(resolution, operand)| {
                    resolution.operand_ordinal == operand.operand_ordinal
                        && resolution.bytes.len() == operand.output.capacity()
                })
            && i64_resolutions.len() == self.i64_operands.len()
            && i64_resolutions
                .iter()
                .zip(&self.i64_operands)
                .all(|(resolution, operand)| resolution.operand_ordinal == operand.operand_ordinal)
    }

    pub(crate) fn reserved_bytes(&self) -> Option<usize> {
        self.byte_operands
            .iter()
            .try_fold(0usize, |total, operand| {
                operand
                    .pre_bytes
                    .len()
                    .checked_mul(2)
                    .and_then(|bytes| total.checked_add(bytes))
            })
    }

    pub(crate) fn initial_rows(
        &self,
    ) -> (
        Vec<FilesystemMutableByteOperand>,
        Vec<FilesystemMutableI64Operand>,
    ) {
        (
            self.byte_operands
                .iter()
                .map(|operand| FilesystemMutableByteOperand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_bytes: operand.pre_bytes.clone(),
                    post_bytes: operand.pre_bytes.clone(),
                })
                .collect(),
            self.i64_operands
                .iter()
                .map(|operand| FilesystemMutableI64Operand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_value: operand.pre_value,
                    post_value: operand.pre_value,
                })
                .collect(),
        )
    }

    pub(crate) fn completed_rows(
        &self,
    ) -> EvalResult<(
        Vec<FilesystemMutableByteOperand>,
        Vec<FilesystemMutableI64Operand>,
    )> {
        let byte_operands = self
            .byte_operands
            .iter()
            .map(|operand| {
                Ok(FilesystemMutableByteOperand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_bytes: operand.pre_bytes.clone(),
                    post_bytes: operand.output.snapshot()?,
                })
            })
            .collect::<EvalResult<Vec<_>>>()?;
        let i64_operands = self
            .i64_operands
            .iter()
            .map(|operand| {
                Ok(FilesystemMutableI64Operand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_value: operand.pre_value,
                    post_value: operand.output.snapshot()?,
                })
            })
            .collect::<EvalResult<Vec<_>>>()?;
        Ok((byte_operands, i64_operands))
    }

    pub(crate) fn apply_replay_post_state(
        &self,
        byte_operands: &[FilesystemMutableByteOperand],
        i64_operands: &[FilesystemMutableI64Operand],
    ) -> EvalResult<()> {
        if byte_operands.len() != self.byte_operands.len()
            || i64_operands.len() != self.i64_operands.len()
        {
            return trap("filesystem replay mutable operand count changed");
        }
        for (prepared, recorded) in self.byte_operands.iter().zip(byte_operands) {
            if prepared.operand_ordinal != recorded.operand_ordinal
                || prepared.pre_bytes != recorded.pre_bytes
                || prepared.output.capacity() != recorded.post_bytes.len()
            {
                return trap("filesystem replay mutable byte operand changed");
            }
            prepared.output.write(&recorded.post_bytes)?;
        }
        for (prepared, recorded) in self.i64_operands.iter().zip(i64_operands) {
            if prepared.operand_ordinal != recorded.operand_ordinal
                || prepared.pre_value != recorded.pre_value
            {
                return trap("filesystem replay mutable scalar operand changed");
            }
            prepared.output.write(recorded.post_value)?;
        }
        Ok(())
    }
}

pub(crate) fn mutable_byte_observation(
    operand_ordinal: u8,
    output: &PreparedByteOutput,
) -> EvalResult<PreparedFilesystemMutableByteObservation> {
    Ok(PreparedFilesystemMutableByteObservation {
        operand_ordinal,
        output: output.clone(),
        pre_bytes: output.snapshot()?,
    })
}

pub(crate) fn mutable_i64_observation(
    operand_ordinal: u8,
    output: &PreparedI64Output,
) -> EvalResult<PreparedFilesystemMutableI64Observation> {
    Ok(PreparedFilesystemMutableI64Observation {
        operand_ordinal,
        output: output.clone(),
        pre_value: output.snapshot()?,
    })
}
