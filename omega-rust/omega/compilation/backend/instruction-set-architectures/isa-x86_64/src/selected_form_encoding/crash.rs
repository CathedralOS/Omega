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

/// The caller must retain an admitted CPL0 execution domain for this form.
pub(crate) fn privileged_declaration(
    constraint: &RegisterInstructionConstraint,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::PrivilegedCrash,
        constraint.key,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ControlFlow,
        MachineAlternativeFamily::PrivilegedCrash,
        MachineSizeKnowledge::ExactBytes(4),
        MachineEncodedEffects::privileged_stop_v1(&constraint.implicit_uses, &constraint.clobbers),
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
    fn privileged_stop_has_exact_bytes_and_control_effects() {
        use selected_instructions::SelectedInstructionKind;
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let kind = SelectedInstructionKind::PrivilegedCrash;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::PrivilegedCrash,
            variant: 0,
        };
        let encoded =
            crate::encode_x86_64_selected_form(&physical, kind, alternative, &[]).unwrap();
        assert_eq!(encoded.bytes(), [0xfa, 0xf4, 0xeb, 0xfd]);
        let units = |name| physical.model().view_named(name).unwrap().units.clone();
        assert_eq!(
            encoded.footprint().encoded,
            MachineEncodedEffects::privileged_stop_v1(&units("rip"), &units("control_flags"))
        );
        for bytes in [
            vec![0x0f, 0x0b],
            vec![0xfa, 0xf4],
            vec![0xfa, 0xf4, 0xeb, 0xfe],
            vec![0xfa, 0xf4, 0xeb, 0xfd, 0x90],
        ] {
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[],
                    &bytes
                )
                .is_err()
            );
        }
        assert!(
            crate::encode_x86_64_selected_form(
                &physical,
                kind,
                alternative,
                &[physical.model().view_named("rax").unwrap().id]
            )
            .is_err()
        );
        assert!(
            crate::validate_x86_64_selected_form_encoding(
                &physical,
                SelectedInstructionKind::Crash,
                alternative,
                &[],
                encoded.bytes()
            )
            .is_err()
        );
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
