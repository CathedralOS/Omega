//! Full-system A64 DMB LD/ST/SY forms. The scope and access mask remain
//! catalog identity, not an AtomicFence promise or a general immediate.
//! Arm A64 ISA, DMB: CRm 1101/1110/1111 selects LD/ST/SY respectively.
//! https://support.arm.com/documentation/ddi0596/2020-12/Base-Instructions/DMB--Data-Memory-Barrier-
use crate::machine_effects::form_alternative;
use register_model::RegisterConstraintKey;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedEffects, MachineLatencyKnowledge, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, MemoryFenceKind,
};

pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::MemoryFence,
        constraint,
        memory: MachineMemoryEffect::CatalogFenceV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: form_alternative!(
            MachineAlternativeFamily::MemoryFence,
            MachineSizeKnowledge::ExactBytes(4),
            MachineEncodedEffects::catalog_fence_v1()
        ),
    }
}

pub(crate) fn encode(kind: MemoryFenceKind) -> Option<Vec<u8>> {
    let option = match kind {
        MemoryFenceKind::Aarch64Load => 0xd,
        MemoryFenceKind::Aarch64Store => 0xe,
        MemoryFenceKind::Aarch64Full => 0xf,
        _ => return None,
    };
    Some((0xd503_30bf_u32 | (option << 8)).to_le_bytes().to_vec())
}

pub(crate) fn decode(bytes: &[u8]) -> Option<MemoryFenceKind> {
    let word = u32::from_le_bytes(bytes.try_into().ok()?);
    if word & !0x0000_0f00 != 0xd503_30bf {
        return None;
    }
    match (word >> 8) & 0xf {
        0xd => Some(MemoryFenceKind::Aarch64Load),
        0xe => Some(MemoryFenceKind::Aarch64Store),
        0xf => Some(MemoryFenceKind::Aarch64Full),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::rejects_every_bit_flip;
    use selected_instructions::{RegisterOperation, SelectedInstructionKind};
    #[test]
    fn catalog_fences_keep_exact_bytes_and_reject_other_effects() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::MemoryFence,
            variant: 0,
        };
        for (kind, expected) in [
            (MemoryFenceKind::Aarch64Load, &[0xbf, 0x3d, 0x03, 0xd5][..]),
            (MemoryFenceKind::Aarch64Store, &[0xbf, 0x3e, 0x03, 0xd5][..]),
            (MemoryFenceKind::Aarch64Full, &[0xbf, 0x3f, 0x03, 0xd5][..]),
        ] {
            let instruction = SelectedInstructionKind::RegisterAccess {
                operation: RegisterOperation::MemoryFence(kind),
            };
            let realized =
                crate::encode_aarch64_selected_form(&physical, instruction, alternative, &[])
                    .unwrap();
            assert_eq!(realized.bytes(), expected);
            assert_eq!(
                realized.footprint().encoded,
                MachineEncodedEffects::catalog_fence_v1()
            );
            assert!(
                realized.footprint().register_reads.is_empty()
                    && realized.footprint().register_writes.is_empty()
            );
            assert!(!realized.footprint().writes_nzcv);
            rejects_every_bit_flip(&physical, instruction, alternative, &[], expected);
            let mut doubled = expected.to_vec();
            doubled.extend(expected);
            for changed in [&doubled[..], &expected[..expected.len() - 1], &[]] {
                assert!(
                    crate::validate_aarch64_selected_form_encoding(
                        &physical,
                        instruction,
                        alternative,
                        &[],
                        changed
                    )
                    .is_err()
                );
            }
            let wrong_kind = SelectedInstructionKind::RegisterAccess {
                operation: RegisterOperation::MemoryFence(MemoryFenceKind::Full),
            };
            assert!(
                crate::encode_aarch64_selected_form(&physical, wrong_kind, alternative, &[])
                    .is_err()
            );
            assert!(
                crate::encode_aarch64_selected_form(
                    &physical,
                    instruction,
                    MachineAlternativeKey {
                        family: MachineAlternativeFamily::AtomicFence,
                        variant: 0
                    },
                    &[]
                )
                .is_err()
            );
            let operand = physical.model().view_named("x0").unwrap().id;
            assert!(
                crate::encode_aarch64_selected_form(
                    &physical,
                    instruction,
                    alternative,
                    &[operand]
                )
                .is_err()
            );
        }
    }
}
