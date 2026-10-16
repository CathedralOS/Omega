//! A64 MRS/MSR system-register transfers. The fixed catalog register is part
//! of the selected kind; the one explicit operand is an allocated 64-bit GPR.
//! Architectural encodings follow Arm's system-register instruction fields,
//! independently pinned by llvm-mc vectors for all nine reads and seven writes.

use super::Aarch64SelectedFormEncodingError;
use register_model::RegisterConstraintKey;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedEffects, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, RegisterOperation, SystemRegister,
};

pub(crate) fn effects(read: bool) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model_ref();
    let scratch = if read {
        &["x9", "x15"][..]
    } else {
        &["x9", "x10", "x11", "x15"][..]
    };
    let mut clobbers = scratch
        .iter()
        .flat_map(|name| {
            physical
                .view_named(name)
                .expect("canonical AArch64 scratch register")
                .units
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    clobbers.sort_unstable();
    clobbers.dedup();
    MachineEncodedEffects {
        implicit_unit_clobbers: clobbers,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            if read { vec![] } else { vec![0] },
            if read { vec![0] } else { vec![] },
        )
    }
}

pub(crate) fn declaration(
    read: bool,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: if read {
            MachineSemanticKind::SystemRegisterRead
        } else {
            MachineSemanticKind::SystemRegisterWrite
        },
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: if read {
                    MachineAlternativeFamily::SystemRegisterRead
                } else {
                    MachineAlternativeFamily::SystemRegisterWrite
                },
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(read),
        }],
    }
}

pub(crate) fn encode(
    operation: RegisterOperation,
    registers: &[u8],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let [operand] = registers else {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    };
    if *operand > 30 || !operation.is_admitted() {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    let (base, register) = match operation {
        RegisterOperation::SystemRead(register) => (0xd520_0000, register),
        RegisterOperation::SystemWrite(register) => (0xd500_0000, register),
        _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
    };
    let selector: u16 = match register {
        SystemRegister::SctlrEl1 => 0xc080,
        SystemRegister::TcrEl1 => 0xc102,
        SystemRegister::Ttbr0El1 => 0xc100,
        SystemRegister::Ttbr1El1 => 0xc101,
        SystemRegister::MairEl1 => 0xc510,
        SystemRegister::VbarEl1 => 0xc600,
        SystemRegister::TpidrEl1 => 0xc684,
        SystemRegister::EsrEl1 => 0xc290,
        SystemRegister::FarEl1 => 0xc300,
    };
    let word = base | (u32::from(selector) << 5) | u32::from(*operand);
    Ok(word.to_le_bytes().to_vec())
}

/// Decode the architectural selector and direction independently of the
/// encoder's requested operation. Unsupported catalog writes remain refusals.
pub(crate) fn decode(word: u32) -> Option<(RegisterOperation, u8)> {
    let read = match word & 0xffe0_0000 {
        0xd520_0000 => true,
        0xd500_0000 => false,
        _ => return None,
    };
    let register = match (word >> 5) & 0xffff {
        0xc080 => SystemRegister::SctlrEl1,
        0xc102 => SystemRegister::TcrEl1,
        0xc100 => SystemRegister::Ttbr0El1,
        0xc101 => SystemRegister::Ttbr1El1,
        0xc510 => SystemRegister::MairEl1,
        0xc600 => SystemRegister::VbarEl1,
        0xc684 => SystemRegister::TpidrEl1,
        0xc290 => SystemRegister::EsrEl1,
        0xc300 => SystemRegister::FarEl1,
        _ => return None,
    };
    let operation = if read {
        RegisterOperation::SystemRead(register)
    } else {
        RegisterOperation::SystemWrite(register)
    };
    operation
        .is_admitted()
        .then_some((operation, (word & 31) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::SelectedInstructionKind;

    #[test]
    fn system_register_transfers_match_llvm_and_reject_changed_instruction_fields() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        // llvm-mc -triple=aarch64 -show-encoding: MRS x19 / MSR x20.
        // Each word is an independent reference, not an encoder-derived oracle.
        let vectors = [
            (
                SystemRegister::SctlrEl1,
                0xd5381013_u32,
                Some(0xd5181014_u32),
            ),
            (SystemRegister::TcrEl1, 0xd5382053, Some(0xd5182054)),
            (SystemRegister::Ttbr0El1, 0xd5382013, Some(0xd5182014)),
            (SystemRegister::Ttbr1El1, 0xd5382033, Some(0xd5182034)),
            (SystemRegister::MairEl1, 0xd538a213, Some(0xd518a214)),
            (SystemRegister::VbarEl1, 0xd538c013, Some(0xd518c014)),
            (SystemRegister::TpidrEl1, 0xd538d093, Some(0xd518d094)),
            (SystemRegister::EsrEl1, 0xd5385213, None),
            (SystemRegister::FarEl1, 0xd5386013, None),
        ];
        for (register, read_word, write_word) in vectors {
            for (read, word) in [(true, Some(read_word)), (false, write_word)] {
                let Some(word) = word else {
                    continue;
                };
                let operation = if read {
                    RegisterOperation::SystemRead(register)
                } else {
                    RegisterOperation::SystemWrite(register)
                };
                let operand = physical
                    .model()
                    .view_named(if read { "x19" } else { "x20" })
                    .unwrap()
                    .id;
                let kind = SelectedInstructionKind::RegisterAccess { operation };
                let alternative = MachineAlternativeKey {
                    family: if read {
                        MachineAlternativeFamily::SystemRegisterRead
                    } else {
                        MachineAlternativeFamily::SystemRegisterWrite
                    },
                    variant: 0,
                };
                let encoded =
                    crate::encode_aarch64_selected_form(&physical, kind, alternative, &[operand])
                        .unwrap();
                assert_eq!(encoded.bytes(), &word.to_le_bytes());
                assert_eq!(encoded.footprint().encoded, effects(read));
                assert_eq!(
                    encoded.footprint().register_reads,
                    if read { vec![] } else { vec![operand] }
                );
                assert_eq!(
                    encoded.footprint().register_writes,
                    if read { vec![operand] } else { vec![] }
                );
                for bit in 0..32 {
                    let changed = (word ^ (1 << bit)).to_le_bytes();
                    assert!(
                        crate::validate_aarch64_selected_form_encoding(
                            &physical,
                            kind,
                            alternative,
                            &[operand],
                            &changed
                        )
                        .is_err(),
                        "{operation:?}, bit {bit}"
                    );
                }
                let mut extended = encoded.bytes().to_vec();
                extended.push(0);
                for bytes in [&extended[..], &encoded.bytes()[..3]] {
                    assert!(
                        crate::validate_aarch64_selected_form_encoding(
                            &physical,
                            kind,
                            alternative,
                            &[operand],
                            bytes
                        )
                        .is_err()
                    );
                }
                let narrow = physical.model().view_named("w19").unwrap().id;
                assert!(
                    crate::encode_aarch64_selected_form(&physical, kind, alternative, &[narrow])
                        .is_err()
                );
                assert!(
                    crate::encode_aarch64_selected_form(&physical, kind, alternative, &[]).is_err()
                );
                assert!(
                    crate::encode_aarch64_selected_form(
                        &physical,
                        kind,
                        MachineAlternativeKey {
                            variant: 1,
                            ..alternative
                        },
                        &[operand]
                    )
                    .is_err()
                );
            }
        }
        for register in [SystemRegister::EsrEl1, SystemRegister::FarEl1] {
            assert!(encode(RegisterOperation::SystemWrite(register), &[20]).is_err());
        }
        assert!(decode(0xd518_5214).is_none()); // No source-catalog ESR_EL1 write.
        assert!(decode(0xd518_6014).is_none()); // No source-catalog FAR_EL1 write.
        assert!(decode(0xd538_0000).is_none()); // Unadmitted selector.
    }
}
