//! Exact operand-free architectural trap for a semantic Crash leaf.
use crate::machine_effects::form_alternative;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedEffects, MachineLatencyKnowledge, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

pub(crate) fn declaration(constraint: &RegisterInstructionConstraint) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::Crash,
        constraint: constraint.key,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::ExplicitCrashV1,
        barrier: MachineBarrier::ControlFlow,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: form_alternative!(
            MachineAlternativeFamily::Crash,
            MachineSizeKnowledge::ExactBytes(4),
            MachineEncodedEffects::crash_v1(&constraint.implicit_uses)
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::selected_form_encoding::ValidatedAarch64SelectedFormEncoding;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, fixture,
    };

    impl fixture::EncodedWord for ValidatedAarch64SelectedFormEncoding {
        fn encoded_bytes(&self) -> &[u8] {
            self.bytes()
        }

        fn encoded_effects(&self) -> &MachineEncodedEffects {
            &self.footprint().encoded
        }
    }

    #[test]
    fn crash_rejects_changed_bytes_operands_and_alternatives() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        fixture::assert_crash_refusals(
            fixture::CrashWord {
                physical: &physical,
                bytes: &[0x00, 0x00, 0x20, 0xd4],
                program_counter: "pc",
                foreign_operand: "x0",
            },
            MachineAlternativeKey {
                family: MachineAlternativeFamily::Crash,
                variant: 0,
            },
            |kind, alternative, operands| {
                crate::encode_aarch64_selected_form(&physical, kind, alternative, operands)
            },
            |kind, alternative, operands, bytes| {
                crate::validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    operands,
                    bytes,
                )
            },
        );
    }
}
