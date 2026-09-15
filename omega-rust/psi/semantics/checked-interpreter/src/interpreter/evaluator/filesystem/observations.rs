//! Recording what a call observed: operands, returned paths, byte regions,
//! prepared operand resolutions, mutable observations and metadata.

use crate::interpreter::evaluator::filesystem::filesystem_calls::checked_observation_evidence_total;
use crate::interpreter::evaluator::{
    EvalResult, Evaluator, FIND_DATA_OUTPUT_BYTES, FilesystemHostOperation, Halt,
    MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES, PreparedByteOutput,
    PreparedFilesystemMutableObservationPlan, STAT_OUTPUT_BYTES, trap,
};
use crate::{
    FilesystemByteOperand, FilesystemMetadataField, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind, FilesystemMutableByteOperandResolution,
    FilesystemMutableI64OperandResolution, FilesystemObservedByteRegion,
    FilesystemObservedByteRegionKind, FilesystemPathLikeOperand, FilesystemReturnedPath,
    FilesystemReturnedPathCompleteness, FilesystemReturnedPathKind,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
};

impl<'program> Evaluator<'program> {
    pub(crate) fn record_operand_observations(
        &mut self,
        attempt_index: usize,
        scalar_operands: Vec<FilesystemScalarOperand>,
        byte_operands: Vec<FilesystemByteOperand>,
        path_like_operands: Vec<FilesystemPathLikeOperand>,
        rooted_path_operand_resolutions: &[FilesystemRootedPathOperandResolution],
        mutable_plan: &PreparedFilesystemMutableObservationPlan,
    ) -> EvalResult<()> {
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        if attempt.scalar_operands != scalar_operands
            || attempt.byte_operands != byte_operands
            || attempt.path_like_operands != path_like_operands
        {
            return Err(Halt::Trap(
                "incremental filesystem operand evidence disagrees with the fully prepared call"
                    .to_owned(),
            ));
        }
        if attempt.rooted_path_operand_resolutions != rooted_path_operand_resolutions {
            return Err(Halt::Trap(
                "incremental filesystem rooted-path resolution evidence disagrees with the fully prepared call"
                    .to_owned(),
            ));
        }
        self.validate_incremental_mutable_operand_resolutions(attempt_index, mutable_plan)?;
        let retained_bytes = mutable_plan.reserved_bytes();
        let Some(next_total) = retained_bytes.and_then(|bytes| {
            checked_observation_evidence_total(self.filesystem_observation_evidence_bytes, bytes)
        }) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        let attempt = &mut self.filesystem_operation_attempts[attempt_index];
        let (mutable_byte_operands, mutable_i64_operands) = mutable_plan.initial_rows();
        attempt.mutable_byte_operands = mutable_byte_operands;
        attempt.mutable_i64_operands = mutable_i64_operands;
        Ok(())
    }

    pub(crate) fn record_returned_path_observation(
        &mut self,
        operand_ordinal: u8,
        kind: FilesystemReturnedPathKind,
        completeness: FilesystemReturnedPathCompleteness,
        bytes: &[u8],
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        let attempt_index = *self
            .filesystem_operation_attempt_stack
            .last()
            .expect("returned-path observation requires an active filesystem attempt");
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .returned_paths
            .push(FilesystemReturnedPath {
                operand_ordinal,
                kind,
                completeness,
                bytes: bytes.to_owned(),
            });
        Ok(())
    }

    pub(crate) fn record_observed_byte_region(
        &mut self,
        output_operand_ordinal: u8,
        kind: FilesystemObservedByteRegionKind,
        output: &PreparedByteOutput,
        offset: usize,
        length: usize,
    ) -> EvalResult<()> {
        let end = offset.checked_add(length).ok_or_else(|| {
            Halt::Trap("filesystem observed-byte region length overflowed".to_owned())
        })?;
        if end > output.capacity() {
            return Err(Halt::Trap(format!(
                "filesystem observed-byte region {offset}..{end} exceeds output capacity {}",
                output.capacity()
            )));
        }
        let attempt_index = *self
            .filesystem_operation_attempt_stack
            .last()
            .expect("observed-byte region requires an active filesystem attempt");
        self.filesystem_operation_attempts[attempt_index]
            .observed_byte_regions
            .push(FilesystemObservedByteRegion {
                output_operand_ordinal,
                kind,
                offset,
                length,
            });
        Ok(())
    }

    fn validate_incremental_mutable_operand_resolutions(
        &self,
        attempt_index: usize,
        plan: &PreparedFilesystemMutableObservationPlan,
    ) -> EvalResult<()> {
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        if plan.matches_resolution_roles(
            &attempt.mutable_byte_operand_resolutions,
            &attempt.mutable_i64_operand_resolutions,
        ) {
            Ok(())
        } else {
            Err(Halt::Trap(
                "incremental filesystem mutable-carrier resolution evidence disagrees with the fully prepared call"
                    .to_owned(),
            ))
        }
    }

    pub(crate) fn record_prepared_filesystem_scalar_operand(
        &mut self,
        attempt_index: usize,
        operand: FilesystemScalarOperand,
    ) {
        self.filesystem_operation_attempts[attempt_index]
            .scalar_operands
            .push(operand);
    }

    pub(crate) fn record_prepared_filesystem_byte_operand(
        &mut self,
        attempt_index: usize,
        operand: FilesystemByteOperand,
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            operand.bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .byte_operands
            .push(operand);
        Ok(())
    }

    pub(crate) fn record_prepared_filesystem_path_like_operand(
        &mut self,
        attempt_index: usize,
        operand: FilesystemPathLikeOperand,
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            operand.bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .path_like_operands
            .push(operand);
        Ok(())
    }

    pub(crate) fn record_prepared_filesystem_rooted_path_operand_resolution(
        &mut self,
        attempt_index: usize,
        resolution: FilesystemRootedPathOperandResolution,
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            resolution.relative_path.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .rooted_path_operand_resolutions
            .push(resolution);
        Ok(())
    }

    pub(crate) fn record_prepared_filesystem_mutable_byte_operand_resolution(
        &mut self,
        attempt_index: usize,
        operand_ordinal: u8,
        output: &PreparedByteOutput,
    ) -> EvalResult<()> {
        let bytes = output.snapshot()?;
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .mutable_byte_operand_resolutions
            .push(FilesystemMutableByteOperandResolution {
                operand_ordinal,
                bytes,
            });
        Ok(())
    }

    pub(crate) fn record_prepared_filesystem_mutable_i64_operand_resolution(
        &mut self,
        attempt_index: usize,
        operand_ordinal: u8,
        value: i64,
    ) {
        self.filesystem_operation_attempts[attempt_index]
            .mutable_i64_operand_resolutions
            .push(FilesystemMutableI64OperandResolution {
                operand_ordinal,
                value,
            });
    }

    pub(crate) fn complete_mutable_observations(
        &mut self,
        attempt_index: usize,
        plan: &PreparedFilesystemMutableObservationPlan,
    ) -> EvalResult<()> {
        let (mutable_byte_operands, mutable_i64_operands) = plan.completed_rows()?;
        let attempt = &mut self.filesystem_operation_attempts[attempt_index];
        attempt.mutable_byte_operands = mutable_byte_operands;
        attempt.mutable_i64_operands = mutable_i64_operands;
        Ok(())
    }

    pub(crate) fn validate_observed_byte_regions(
        &self,
        attempt_index: usize,
        operation: FilesystemHostOperation,
        result: i64,
    ) -> EvalResult<()> {
        let expected = match operation {
            FilesystemHostOperation::Read if result >= 0 => usize::try_from(result)
                .ok()
                .map(|length| (FilesystemObservedByteRegionKind::SequentialFileRead, length)),
            FilesystemHostOperation::ReadAt if result >= 0 => usize::try_from(result)
                .ok()
                .map(|length| (FilesystemObservedByteRegionKind::PositionedFileRead, length)),
            FilesystemHostOperation::ReadDir if result >= 0 => usize::try_from(result)
                .ok()
                .map(|length| (FilesystemObservedByteRegionKind::DirectoryRecords, length)),
            FilesystemHostOperation::FindFirst if result >= 0 => Some((
                FilesystemObservedByteRegionKind::FindEntry,
                FIND_DATA_OUTPUT_BYTES,
            )),
            FilesystemHostOperation::FindNext if result == 1 => Some((
                FilesystemObservedByteRegionKind::FindEntry,
                FIND_DATA_OUTPUT_BYTES,
            )),
            FilesystemHostOperation::FindNext if result == 0 => {
                Some((FilesystemObservedByteRegionKind::FindEntry, 0))
            }
            _ => None,
        };
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        let Some((expected_kind, expected_length)) = expected else {
            if attempt.observed_byte_regions.is_empty() {
                return Ok(());
            }
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an unexpected observed-byte region"
            )));
        };

        let [region] = &attempt.observed_byte_regions[..] else {
            return Err(Halt::Trap(format!(
                "successful canonical filesystem operation `{operation}` did not retain exactly one observed-byte region"
            )));
        };
        if region.kind != expected_kind
            || region.output_operand_ordinal != 1
            || region.offset != 0
            || region.length != expected_length
        {
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an inconsistent observed-byte region"
            )));
        }
        let Some(output) = attempt
            .mutable_byte_operands
            .iter()
            .find(|output| output.operand_ordinal == region.output_operand_ordinal)
        else {
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an observed-byte region without its output carrier"
            )));
        };
        let end = region.offset.checked_add(region.length).ok_or_else(|| {
            Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an overflowing observed-byte region"
            ))
        })?;
        if end > output.post_bytes.len() {
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an out-of-bounds observed-byte region"
            )));
        }
        Ok(())
    }

    pub(crate) fn validate_metadata_observations(
        &self,
        attempt_index: usize,
        operation: FilesystemHostOperation,
        result: i64,
    ) -> EvalResult<()> {
        let expected_kind = match (operation, result) {
            (FilesystemHostOperation::ReadMetadata, 0) => {
                Some(FilesystemMetadataObservationKind::FollowedPath)
            }
            (FilesystemHostOperation::ReadFileMetadata, 0) => {
                Some(FilesystemMetadataObservationKind::OpenDescriptor)
            }
            (FilesystemHostOperation::ReadSymlinkMetadata, 0) => {
                Some(FilesystemMetadataObservationKind::UnfollowedFinalPath)
            }
            _ => None,
        };
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        let Some(expected_kind) = expected_kind else {
            if attempt.metadata_observations.is_empty() {
                return Ok(());
            }
            return trap(format!(
                "canonical filesystem operation `{operation}` retained an unexpected metadata observation"
            ));
        };
        let [observation] = &attempt.metadata_observations[..] else {
            return trap(format!(
                "successful canonical filesystem operation `{operation}` did not retain exactly one metadata observation"
            ));
        };
        if observation.kind() != expected_kind || observation.output_operand_ordinal() != 1 {
            return trap(format!(
                "canonical filesystem operation `{operation}` retained an inconsistent metadata observation"
            ));
        }
        let Some(output) = attempt
            .mutable_byte_operands
            .iter()
            .find(|output| output.operand_ordinal == observation.output_operand_ordinal())
        else {
            return trap(format!(
                "canonical filesystem operation `{operation}` retained metadata without its output carrier"
            ));
        };
        let expected_carrier = self.canonical_metadata_carrier(
            *observation,
            output.post_bytes.len(),
            "retained metadata",
        )?;
        if output.post_bytes != expected_carrier {
            return trap(format!(
                "canonical filesystem operation `{operation}` metadata carrier disagrees with its selected-target semantic row"
            ));
        }
        Ok(())
    }

    pub(crate) fn canonical_metadata_carrier(
        &self,
        observation: FilesystemMetadataObservation,
        capacity: usize,
        context: &str,
    ) -> EvalResult<Vec<u8>> {
        if capacity < STAT_OUTPUT_BYTES || self.filesystem_metadata_layout.record_size() > capacity
        {
            return trap(format!(
                "{context} requires the {STAT_OUTPUT_BYTES}-byte filesystem metadata API carrier and selected {}-byte record, but holds {capacity} bytes",
                self.filesystem_metadata_layout.record_size()
            ));
        }
        let mut carrier = vec![0u8; capacity];
        for field in FilesystemMetadataField::ALL {
            let placement = self.filesystem_metadata_layout.field_layout(field);
            let width = usize::from(placement.stored_width_bits() / 8);
            let bytes = if let Some(value) = observation.unsigned_field(field) {
                let maximum = match placement.stored_width_bits() {
                    16 => u64::from(u16::MAX),
                    32 => u64::from(u32::MAX),
                    64 => u64::MAX,
                    _ => unreachable!("validated metadata stored width"),
                };
                if value > maximum {
                    return trap(format!(
                        "filesystem metadata field {field:?} value {value} exceeds its selected {}-bit carrier",
                        placement.stored_width_bits()
                    ));
                }
                value.to_le_bytes()
            } else {
                let value = observation
                    .signed_field(field)
                    .expect("metadata field is either signed or unsigned");
                let fits = match placement.stored_width_bits() {
                    16 => i16::try_from(value).is_ok(),
                    32 => i32::try_from(value).is_ok(),
                    64 => true,
                    _ => unreachable!("validated metadata stored width"),
                };
                if !fits {
                    return trap(format!(
                        "filesystem metadata field {field:?} value {value} exceeds its selected {}-bit carrier",
                        placement.stored_width_bits()
                    ));
                }
                value.to_le_bytes()
            };
            let start = placement.offset();
            carrier[start..start + width].copy_from_slice(&bytes[..width]);
        }
        Ok(carrier)
    }
}
