//! Optimizer module role: stage group. Historical scalar selected-plan roster.

use selected_instructions::SelectedInstructionPlan;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::FixedViewCopyDecodeError;

use super::{
    super::{
        primitives::{Cursor, decode_id, length},
        values::{decode_target, encode_target},
    },
    function::{decode_function, encode_function},
};

pub(super) fn encode(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    length(bytes, plan.functions.len());
    for function in &plan.functions {
        encode_function(bytes, function);
    }
}

pub(super) fn decode(
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
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        functions.push(decode_function(cursor)?);
    }
    Ok(SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: marker,
            program_fingerprint: fingerprint,
        },
        fuel_schedule,
        target,
        entry,
        functions,
        structural_unit_functions: Vec::new(),
        projected_structural_call_returns: Vec::new(),
    })
}
