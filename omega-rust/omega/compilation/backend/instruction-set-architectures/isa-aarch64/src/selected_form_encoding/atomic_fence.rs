//! A full coherent-memory barrier, independent of cell width or value storage.
use register_model::RegisterConstraintKey;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedEffects, MachineLatencyKnowledge, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::AtomicFence,
        constraint,
        memory: MachineMemoryEffect::AtomicFenceV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicFence,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::atomic_fence_v1(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::rejects_every_bit_flip;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        SelectedInstructionKind,
    };

    #[test]
    fn atomic_fence_pins_full_barrier_bytes_and_rejects_substitutions() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let kind = SelectedInstructionKind::AtomicFence;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::AtomicFence,
            variant: 0,
        };
        let encoded =
            crate::encode_aarch64_selected_form(&physical, kind, alternative, &[]).unwrap();
        assert_eq!(encoded.bytes(), &[0xbf, 0x3f, 0x03, 0xd5]);
        assert_eq!(
            encoded.footprint().encoded,
            MachineEncodedEffects::atomic_fence_v1()
        );
        rejects_every_bit_flip(&physical, kind, alternative, &[], encoded.bytes());
        let mut extended = encoded.bytes().to_vec();
        extended.push(0);
        for bytes in [&extended[..], &encoded.bytes()[..encoded.bytes().len() - 1]] {
            assert!(
                crate::validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[],
                    bytes
                )
                .is_err()
            );
        }
        let operand = physical.model().view_named("x0").unwrap().id;
        assert!(
            crate::encode_aarch64_selected_form(&physical, kind, alternative, &[operand]).is_err()
        );
        assert!(
            crate::encode_aarch64_selected_form(
                &physical,
                kind,
                MachineAlternativeKey {
                    variant: 1,
                    ..alternative
                },
                &[]
            )
            .is_err()
        );
        assert!(
            crate::encode_aarch64_selected_form(
                &physical,
                kind,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Crash,
                    variant: 0
                },
                &[]
            )
            .is_err()
        );
    }
}
