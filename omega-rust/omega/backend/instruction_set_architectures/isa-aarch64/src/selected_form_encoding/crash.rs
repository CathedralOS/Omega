//! Exact operand-free architectural trap for a semantic Crash leaf.
use register_model::{RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
};

pub(crate) fn effects(program_counter: &[RegisterUnitId]) -> MachineEncodedEffects {
    MachineEncodedEffects {
        // Like a branch, an exception records its site and transfers control;
        // no caller-owned value or storage is returned or cleaned up.
        implicit_unit_uses: program_counter.to_vec(),
        implicit_unit_defs: program_counter.to_vec(),
        trap: MachineEncodedTrapBehavior::ExplicitCrashV1,
        control: MachineEncodedControlEffect::CrashV1,
        ..MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new())
    }
}

pub(crate) fn declaration(constraint: &RegisterInstructionConstraint) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::Crash,
        constraint: constraint.key,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::ExplicitCrashV1,
        barrier: MachineBarrier::ControlFlow,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::Crash,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(&constraint.implicit_uses),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::{MachineAlternativeFamily, MachineAlternativeKey, effects};
    use selected_instructions::SelectedInstructionKind;

    #[test]
    fn crash_rejects_changed_bytes_operands_and_alternatives() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::Crash,
            variant: 0,
        };
        let encode = |physical, alternative, operands| {
            crate::encode_aarch64_selected_form(
                physical,
                SelectedInstructionKind::Crash,
                alternative,
                operands,
            )
        };
        let validate = |physical, alternative, operands, bytes| {
            crate::validate_aarch64_selected_form_encoding(
                physical,
                SelectedInstructionKind::Crash,
                alternative,
                operands,
                bytes,
            )
        };
        let encoded = crate::encode_aarch64_selected_form(
            &physical,
            SelectedInstructionKind::Crash,
            alternative,
            &[],
        )
        .unwrap();
        assert_eq!(encoded.bytes(), &[0x00, 0x00, 0x20, 0xd4]);
        assert_eq!(
            encoded.footprint().encoded,
            effects(&physical.model().view_named("pc").unwrap().units)
        );
        for bit in 0..32 {
            let mut bytes = encoded.bytes().to_vec();
            bytes[bit / 8] ^= 1 << (bit % 8);
            assert!(
                crate::validate_aarch64_selected_form_encoding(
                    &physical,
                    SelectedInstructionKind::Crash,
                    alternative,
                    &[],
                    &bytes,
                )
                .is_err()
            );
        }
        let mut extended = encoded.bytes().to_vec();
        extended.push(0);
        for bytes in [&extended[..], &encoded.bytes()[..3]] {
            assert!(validate(&physical, alternative, &[], bytes).is_err());
        }
        let view = physical.model().view_named("x0").unwrap().id;
        let unexpected_operands = [view];
        assert!(encode(&physical, alternative, &unexpected_operands).is_err());
        assert!(
            encode(
                &physical,
                MachineAlternativeKey {
                    variant: 1,
                    ..alternative
                },
                &[]
            )
            .is_err()
        );
        assert!(
            encode(
                &physical,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::ReturnUnit,
                    variant: 0
                },
                &[]
            )
            .is_err()
        );
        assert!(
            crate::validate_aarch64_selected_form_encoding(
                &physical,
                SelectedInstructionKind::ReturnUnit,
                alternative,
                &[],
                encoded.bytes(),
            )
            .is_err()
        );
    }
}
