//! Linux supervisor entries use the ordinary external-call operand transport.
//!
//! The selected call roster supplies the exact nonnegative kernel number. This
//! ISA owner materializes that number and enters the kernel; it does not stage
//! arguments, manufacture an import relocation, or infer a source pattern.
//! Fixed-width materialization keeps layout independent of the number. Replay
//! decodes the number and instruction fields separately from byte production.

use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
};
use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use target::NativeTarget;

/// Encode the retained kernel number after ordinary argument placement.
pub fn encode_aarch64_selected_system_call_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let mut bytes = Vec::with_capacity(20);
    for halfword in 0..4 {
        let immediate = ((number as u64 >> (halfword * 16)) & 0xffff) as u32;
        let opcode = if halfword == 0 {
            0xd280_0000
        } else {
            0xf280_0000
        };
        let word = opcode | (halfword << 21) | (immediate << 5) | 8;
        crate::codec_bytes::push_encoded(&mut bytes, word.to_le_bytes());
    }
    crate::codec_bytes::push_encoded(&mut bytes, 0xd400_0001u32.to_le_bytes());
    validate_aarch64_selected_system_call_form(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        operand_accesses,
        number,
        &bytes,
    )
}

/// Decode the materialized number and entry opcode, then reconstruct the
/// canonical ABI footprint without trusting producer-supplied effects.
pub fn validate_aarch64_selected_system_call_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    if target != NativeTarget::linux_arm64()
        || number < 0
        || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
        || !matches!(kind, SelectedInstructionKind::NormalizedForeignCall { .. })
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            })
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if bytes.len() != 20 {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let mut decoded_number = 0u64;
    for (halfword, chunk) in bytes[..16].as_chunks::<4>().0.iter().enumerate() {
        let word = u32::from_le_bytes(*chunk);
        let opcode = if halfword == 0 {
            0xd280_0000
        } else {
            0xf280_0000
        };
        // Match width, opcode, halfword position, and X8; only imm16 varies.
        if word & !0x001f_ffe0 != opcode | ((halfword as u32) << 21) | 8 {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        decoded_number |= u64::from((word >> 5) & 0xffff) << (halfword * 16);
    }
    if bytes[16..] != [0x01, 0x00, 0x00, 0xd4] || decoded_number != number as u64 {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let catalog = crate::aarch64_register_constraint_catalog(physical);
    let keys = crate::aarch64_linux_system_call_keys();
    let row = catalog
        .constraints
        .iter()
        .find(|row| {
            keys.contains(&row.key)
                && row.operands.len() == operand_views.len()
                && row.operands.len() == operand_accesses.len()
                && row
                    .operands
                    .iter()
                    .zip(operand_views.iter().zip(operand_accesses))
                    .all(|(operand, (view, access))| {
                        operand.fixed_view == Some(*view) && operand.access == *access
                    })
        })
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    let effects = MachineEncodedEffects {
        external_operand_reads: row.operand_reads(),
        external_operand_writes: row.operand_writes(),
        implicit_unit_uses: row.implicit_uses.clone(),
        implicit_unit_defs: row.implicit_defs.clone(),
        implicit_unit_clobbers: row.clobbers.clone(),
        memory: MachineEncodedMemoryEffect::NoneV1,
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        control: MachineEncodedControlEffect::SupervisorCallV1,
    };
    let views_for = |access| {
        operand_views
            .iter()
            .zip(operand_accesses)
            .filter_map(|(view, actual)| (*actual == access).then_some(*view))
            .collect()
    };
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: views_for(RegisterOperandAccess::Use),
            register_writes: views_for(RegisterOperandAccess::Def),
            writes_nzcv: true,
            encoded: effects,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::BoundaryMachineId;

    #[test]
    fn system_call_encoding_replays_all_transport_rows_and_rejects_substituted_bytes() {
        let physical = crate::validated_aarch64_physical_register_model();
        let target = NativeTarget::linux_arm64();
        let catalog = crate::aarch64_register_constraint_catalog(physical);
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        for key in crate::aarch64_linux_system_call_keys() {
            let row = catalog.constraint(key).unwrap();
            let views: Vec<_> = row
                .operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect();
            let accesses: Vec<_> = row.operands.iter().map(|operand| operand.access).collect();
            for number in [0, 211, 0x0123_4567_89ab_cdef, i64::MAX] {
                let encoded = encode_aarch64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                    &accesses,
                    number,
                )
                .unwrap();
                let replay = |bytes: &[u8]| {
                    validate_aarch64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                        &accesses,
                        number,
                        bytes,
                    )
                };
                assert_eq!(replay(encoded.bytes()).unwrap(), encoded);
                assert_eq!(
                    encoded.footprint().encoded.implicit_unit_clobbers,
                    row.clobbers
                );
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::SupervisorCallV1
                );
                assert_eq!(
                    encoded.footprint().encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
                // Exhaust the bits of one representative form, not permutations of source syntax.
                if number == 211 && row.operands.len() == 7 {
                    for position in 0..encoded.bytes().len() {
                        for bit in 0..8 {
                            let mut forged = encoded.bytes().to_vec();
                            forged[position] ^= 1 << bit;
                            assert!(
                                replay(&forged).is_err(),
                                "accepted byte {position}, bit {bit}"
                            );
                        }
                    }
                }
                assert!(replay(&encoded.bytes()[1..]).is_err());
                assert!(
                    validate_aarch64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                        &accesses,
                        number ^ 1,
                        encoded.bytes(),
                    )
                    .is_err()
                );
                assert!(
                    validate_aarch64_selected_system_call_form(
                        NativeTarget::macos_arm64(),
                        physical,
                        kind,
                        alternative,
                        &views,
                        &accesses,
                        number,
                        encoded.bytes(),
                    )
                    .is_err()
                );
            }
            assert!(
                encode_aarch64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                    &accesses,
                    -1,
                )
                .is_err()
            );
            if !views.is_empty() {
                let mut forged_views = views.clone();
                forged_views[0] = physical.model().view_named("x16").unwrap().id;
                assert!(
                    encode_aarch64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &forged_views,
                        &accesses,
                        211,
                    )
                    .is_err()
                );
            }
        }
    }
}
