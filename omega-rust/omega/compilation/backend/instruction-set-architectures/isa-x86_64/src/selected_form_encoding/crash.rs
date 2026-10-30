//! Exact operand-free architectural trap for a semantic Crash leaf.
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    MachineAlternativeFamily, MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

pub(crate) fn declaration(constraint: &RegisterInstructionConstraint) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::Crash,
        constraint.key,
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
    use crate::selected_form_encoding::ValidatedX86_64SelectedFormEncoding;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, fixture,
    };

    impl fixture::EncodedWord for ValidatedX86_64SelectedFormEncoding {
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
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        fixture::assert_crash_refusals(
            fixture::CrashWord {
                physical: &physical,
                bytes: &[0x0f, 0x0b],
                program_counter: "rip",
                foreign_operand: "rax",
            },
            MachineAlternativeKey {
                family: MachineAlternativeFamily::Crash,
                variant: 0,
            },
            |kind, alternative, operands| {
                crate::encode_x86_64_selected_form(&physical, kind, alternative, operands)
            },
            |kind, alternative, operands, bytes| {
                crate::validate_x86_64_selected_form_encoding(
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
