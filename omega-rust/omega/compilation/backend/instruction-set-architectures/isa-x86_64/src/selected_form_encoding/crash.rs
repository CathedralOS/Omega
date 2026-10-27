//! Exact operand-free architectural trap for a semantic Crash leaf.
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    MachineAlternativeFamily, MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

pub(crate) fn declaration(constraint: &RegisterInstructionConstraint) -> MachineEffectDeclaration {
    super::alternative_declaration(
        constraint.key,
        MachineSemanticKind::Crash,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::ExplicitCrashV1,
        MachineBarrier::ControlFlow,
        MachineAlternativeFamily::Crash,
        MachineSizeKnowledge::ExactBytes(2),
        MachineEncodedEffects::crash_v1(&constraint.implicit_uses),
    )
}

#[cfg(test)]
mod tests {
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        SelectedInstructionKind,
    };

    #[test]
    fn crash_rejects_changed_bytes_operands_and_alternatives() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::Crash,
            variant: 0,
        };
        let encode = |physical, alternative, operands| {
            crate::encode_x86_64_selected_form(
                physical,
                SelectedInstructionKind::Crash,
                alternative,
                operands,
            )
        };
        let validate = |physical, alternative, operands, bytes| {
            crate::validate_x86_64_selected_form_encoding(
                physical,
                SelectedInstructionKind::Crash,
                alternative,
                operands,
                bytes,
            )
        };
        let encoded = crate::encode_x86_64_selected_form(
            &physical,
            SelectedInstructionKind::Crash,
            alternative,
            &[],
        )
        .unwrap();
        assert_eq!(encoded.bytes(), &[0x0f, 0x0b]);
        assert_eq!(
            encoded.footprint().encoded,
            MachineEncodedEffects::crash_v1(&physical.model().view_named("rip").unwrap().units)
        );
        for bit in 0..16 {
            let mut bytes = encoded.bytes().to_vec();
            bytes[bit / 8] ^= 1 << (bit % 8);
            assert!(
                crate::validate_x86_64_selected_form_encoding(
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
        for bytes in [&extended[..], &encoded.bytes()[..1]] {
            assert!(validate(&physical, alternative, &[], bytes).is_err());
        }
        let view = physical.model().view_named("rax").unwrap().id;
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
            crate::validate_x86_64_selected_form_encoding(
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
