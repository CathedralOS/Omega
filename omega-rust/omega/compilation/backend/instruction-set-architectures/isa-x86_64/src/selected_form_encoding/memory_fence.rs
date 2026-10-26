//! Exact x86 LFENCE/SFENCE/MFENCE forms. The decoder retains the access
//! mask independently of construction; no full fence replaces a partial one.
//! Intel SDM volume 2, LFENCE/MFENCE/SFENCE instruction entries.
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
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::MemoryFence,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(3),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::catalog_fence_v1(),
        }],
    }
}

pub(crate) fn encode(kind: MemoryFenceKind) -> Option<Vec<u8>> {
    let opcode = match kind {
        MemoryFenceKind::Load => 0xe8,
        MemoryFenceKind::Store => 0xf8,
        MemoryFenceKind::Full => 0xf0,
        _ => return None,
    };
    Some(vec![0x0f, 0xae, opcode])
}

pub(crate) fn decode(bytes: &[u8]) -> Option<MemoryFenceKind> {
    match bytes {
        [0x0f, 0xae, 0xe8] => Some(MemoryFenceKind::Load),
        [0x0f, 0xae, 0xf8] => Some(MemoryFenceKind::Store),
        [0x0f, 0xae, 0xf0] => Some(MemoryFenceKind::Full),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{RegisterOperation, SelectedInstructionKind};
    #[test]
    fn catalog_fences_keep_exact_bytes_and_reject_other_effects() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::MemoryFence,
            variant: 0,
        };
        for (kind, expected) in [
            (MemoryFenceKind::Load, &[0x0f, 0xae, 0xe8][..]),
            (MemoryFenceKind::Store, &[0x0f, 0xae, 0xf8][..]),
            (MemoryFenceKind::Full, &[0x0f, 0xae, 0xf0][..]),
        ] {
            let instruction = SelectedInstructionKind::RegisterAccess {
                operation: RegisterOperation::MemoryFence(kind),
            };
            let realized =
                crate::encode_x86_64_selected_form(&physical, instruction, alternative, &[])
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
            assert!(!realized.footprint().writes_rflags);
            for bit in 0..expected.len() * 8 {
                let mut changed = expected.to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
                        &physical,
                        instruction,
                        alternative,
                        &[],
                        &changed
                    )
                    .is_err(),
                    "changed bit {bit}"
                );
            }
            let mut doubled = expected.to_vec();
            doubled.extend(expected);
            for changed in [&doubled[..], &expected[..expected.len() - 1], &[]] {
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
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
                operation: RegisterOperation::MemoryFence(MemoryFenceKind::Aarch64Full),
            };
            assert!(
                crate::encode_x86_64_selected_form(&physical, wrong_kind, alternative, &[])
                    .is_err()
            );
            assert!(
                crate::encode_x86_64_selected_form(
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
            let operand = physical.model().view_named("rax").unwrap().id;
            assert!(
                crate::encode_x86_64_selected_form(&physical, instruction, alternative, &[operand])
                    .is_err()
            );
        }
    }
}
