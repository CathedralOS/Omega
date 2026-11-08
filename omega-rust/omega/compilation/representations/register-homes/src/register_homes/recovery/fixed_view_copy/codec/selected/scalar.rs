//! Optimizer module role: stage group. Historical scalar selected-plan roster.

use selected_instructions::SelectedInstructionPlan;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::FixedViewCopyDecodeError;

use super::{
    super::{
        primitives::{Cursor, decode_id, decode_list, length},
        values::{decode_target, encode_target},
    },
    function::{decode_function, encode_function},
};

pub(in crate::register_homes::recovery::fixed_view_copy::codec) fn encode(
    bytes: &mut Vec<u8>,
    plan: &SelectedInstructionPlan,
) -> Result<(), terminal_codec::CodecError> {
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(bytes, plan.psi.vocabulary_marker.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, plan.fuel_schedule.marker().to_le_bytes());
    encode_target(bytes, plan.target);
    crate::codec_bytes::push_encoded(bytes, plan.entry.get().to_le_bytes());
    length(bytes, plan.functions.len());
    for function in &plan.functions {
        encode_function(bytes, function)?;
    }
    Ok(())
}

pub(in crate::register_homes::recovery::fixed_view_copy::codec) fn decode(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionPlan, FixedViewCopyDecodeError> {
    let fingerprint = SemanticFingerprint::from_bytes(cursor.array()?);
    let marker_raw = cursor.u16()?;
    let marker = VocabularyMarker::new(marker_raw)
        .ok_or(FixedViewCopyDecodeError::InvalidVocabulary(marker_raw))?;
    let fuel_raw = cursor.u32()?;
    let fuel_schedule = FuelScheduleIdentity::new(fuel_raw)
        .ok_or(FixedViewCopyDecodeError::InvalidFuelSchedule(fuel_raw))?;
    let target = decode_target(cursor)?;
    let entry = decode_id(cursor, MachineId::new)?;
    let functions = decode_list(cursor, decode_function)?;
    Ok(SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: marker,
            program_fingerprint: fingerprint,
        },
        fuel_schedule,
        target,
        entry,
        functions: functions.into(),
    })
}
