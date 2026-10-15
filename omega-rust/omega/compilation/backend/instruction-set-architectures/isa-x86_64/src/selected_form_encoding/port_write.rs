//! One checked immediate OUT envelope, shared with final-image port custody.
//! Privilege and selected-provider authority are admitted separately; this
//! owner fixes the actual bytes, scratch registers and architectural fault.
use super::{ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError};
use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedEffects, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, SelectedInstructionKind,
};

pub(crate) fn effects() -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model_ref();
    let mut clobbers = ["rax", "rdx", "r10", "r11"]
        .into_iter()
        .flat_map(|name| {
            physical
                .view_named(name)
                .expect("canonical x86 register")
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
        ..MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new())
    }
}

pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::PortWrite,
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::PortWrite,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(x86_encoding::IMMEDIATE_PORT_WRITE_WIDTH as u16),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(),
        }],
    }
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    port: u16,
    value: u8,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let kind = SelectedInstructionKind::PortWrite { port, value };
    super::request_validation::validate_request(physical, kind, alternative, operands)?;
    if bytes != x86_encoding::encode_immediate_port_write(port, value) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: super::decoding::footprint(kind, alternative, operands),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_write_retains_its_exact_operands_and_rejects_changed_bytes() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::PortWrite,
            variant: 0,
        };
        for (port, value) in [(0, 0), (0x3f8, 0x41), (u16::MAX, u8::MAX)] {
            let kind = SelectedInstructionKind::PortWrite { port, value };
            let encoded =
                crate::encode_x86_64_selected_form(&physical, kind, alternative, &[]).unwrap();
            assert_eq!(
                encoded.bytes(),
                x86_encoding::encode_immediate_port_write(port, value)
            );
            assert_eq!(encoded.footprint().encoded, effects());
            for bit in 0..encoded.bytes().len() * 8 {
                let mut changed = encoded.bytes().to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(validate(&physical, port, value, alternative, &[], &changed).is_err());
            }
            assert!(
                validate(
                    &physical,
                    port,
                    value,
                    alternative,
                    &[],
                    &encoded.bytes()[1..]
                )
                .is_err()
            );
            let mut extended = encoded.bytes().to_vec();
            extended.push(0);
            assert!(validate(&physical, port, value, alternative, &[], &extended).is_err());
            let register = physical.model().view_named("rax").unwrap().id;
            assert!(
                crate::encode_x86_64_selected_form(&physical, kind, alternative, &[register])
                    .is_err()
            );
            assert!(
                crate::encode_x86_64_selected_form(
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
        }
    }
}
