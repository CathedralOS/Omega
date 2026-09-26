//! Rooted output custody and checked metadata carrier encoding.
use crate::interpreter::evaluator::filesystem::filesystem_calls::checked_observation_evidence_total;
use crate::interpreter::evaluator::{
    EvalResult, Evaluator, Halt, MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES, STAT_OUTPUT_BYTES, trap,
};
use crate::{
    FilesystemMetadataField, FilesystemMetadataObservation, FilesystemRootedPathOperandResolution,
};

impl<'program> Evaluator<'program> {
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
