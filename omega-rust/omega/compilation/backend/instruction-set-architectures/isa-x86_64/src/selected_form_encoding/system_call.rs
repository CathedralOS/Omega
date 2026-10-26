//! Linux supervisor entries use the ordinary external-call operand transport.
//!
//! The selected call roster supplies the exact nonnegative kernel number. This
//! ISA owner materializes that number and enters the kernel; it does not stage
//! arguments, manufacture an import relocation, or infer a source pattern.
//! Fixed-width materialization keeps layout independent of the number. Replay
//! decodes the number and instruction fields separately from byte production.

use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    X86_64SelectedFormFootprint,
};
use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use target::NativeTarget;

/// Encode the retained kernel number after ordinary argument placement.
pub fn encode_x86_64_selected_system_call_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let mut bytes = Vec::with_capacity(12);
    bytes.extend_from_slice(&[0x48, 0xb8]);
    crate::codec_bytes::push_encoded(&mut bytes, number.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]);
    validate_x86_64_selected_system_call_form(
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
pub fn validate_x86_64_selected_system_call_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if target != NativeTarget::linux_x64()
        || number < 0
        || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
        || !matches!(kind, SelectedInstructionKind::NormalizedForeignCall { .. })
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            })
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if bytes.len() != 12 || bytes[..2] != [0x48, 0xb8] || bytes[10..] != [0x0f, 0x05] {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let immediate: [u8; 8] = bytes[2..10]
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    if i64::from_le_bytes(immediate) != number {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let catalog = crate::x86_64_register_constraint_catalog_for(physical);
    let keys = crate::x86_64_linux_system_call_keys();
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
        .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
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
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: views_for(RegisterOperandAccess::Use),
            register_writes: views_for(RegisterOperandAccess::Def),
            writes_rflags: true,
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
        let physical = crate::validated_x86_64_physical_register_model();
        let target = NativeTarget::linux_x64();
        let catalog = crate::x86_64_register_constraint_catalog_for(physical);
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        for key in crate::x86_64_linux_system_call_keys() {
            let row = catalog.constraint(key).unwrap();
            let views: Vec<_> = row
                .operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect();
            let accesses: Vec<_> = row.operands.iter().map(|operand| operand.access).collect();
            for number in [0, 211, 0x0123_4567_89ab_cdef, i64::MAX] {
                let encoded = encode_x86_64_selected_system_call_form(
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
                    validate_x86_64_selected_system_call_form(
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
                    validate_x86_64_selected_system_call_form(
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
                    validate_x86_64_selected_system_call_form(
                        NativeTarget::windows_x64(),
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
                encode_x86_64_selected_system_call_form(
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
                forged_views[0] = physical.model().view_named("rsp").unwrap().id;
                assert!(
                    encode_x86_64_selected_system_call_form(
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
