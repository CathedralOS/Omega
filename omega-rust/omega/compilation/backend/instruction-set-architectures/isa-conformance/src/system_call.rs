//! One supervisor-call body over each instruction set's transport rows.
//!
//! The roster sweep, the replay, the control and stack effects, the exhaustive
//! single-bit forgery of one representative form and every refusal a kernel
//! entry owes are stated once here; an instruction set answers with its rows,
//! its bytes and the target it must refuse.

use register_model::{
    RegisterConstraintFamily, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterViewId, ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    MachineCallEffect, MachineEffectCatalog, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineSemanticKind,
    validate_machine_effect_catalog,
};

/// What one validated supervisor-call encoding reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Encoding {
    pub bytes: Vec<u8>,
    pub effects: MachineEncodedEffects,
}

/// Sweep every transport row and kernel number, replay each encoding, and
/// require every substituted byte, number, target and view to be refused.
/// `encode` and `validate` bind the admitted target; `validate_elsewhere`
/// replays under a target the instruction set must refuse.
pub fn conformance<E>(
    rows: &[&RegisterInstructionConstraint],
    forged_view: RegisterViewId,
    encode: impl Fn(&[RegisterViewId], &[RegisterOperandAccess], i64) -> Result<Encoding, E>,
    validate: impl Fn(&[RegisterViewId], &[RegisterOperandAccess], i64, &[u8]) -> Result<Encoding, E>,
    validate_elsewhere: impl Fn(&[RegisterViewId], &[RegisterOperandAccess], i64, &[u8]) -> bool,
) where
    E: std::fmt::Debug,
{
    for row in rows {
        let views: Vec<_> = row
            .operands
            .iter()
            .map(|operand| operand.fixed_view.unwrap())
            .collect();
        let accesses: Vec<_> = row.operands.iter().map(|operand| operand.access).collect();
        for number in [0, 211, 0x0123_4567_89ab_cdef, i64::MAX] {
            let encoded = encode(&views, &accesses, number).unwrap();
            let replay = |bytes: &[u8]| validate(&views, &accesses, number, bytes);
            assert_eq!(replay(&encoded.bytes).unwrap(), encoded);
            assert_eq!(encoded.effects.implicit_unit_clobbers, row.clobbers);
            assert_eq!(
                encoded.effects.control,
                MachineEncodedControlEffect::SupervisorCallV1
            );
            assert_eq!(
                encoded.effects.stack,
                MachineEncodedStackEffect::UnchangedV1
            );
            // Exhaust the bits of one representative form, not permutations of source syntax.
            if number == 211 && row.operands.len() == 7 {
                for position in 0..encoded.bytes.len() {
                    for bit in 0..8 {
                        let mut forged = encoded.bytes.clone();
                        forged[position] ^= 1 << bit;
                        assert!(
                            replay(&forged).is_err(),
                            "accepted byte {position}, bit {bit}"
                        );
                    }
                }
            }
            assert!(replay(&encoded.bytes[1..]).is_err());
            assert!(validate(&views, &accesses, number ^ 1, &encoded.bytes).is_err());
            assert!(validate_elsewhere(
                &views,
                &accesses,
                number,
                &encoded.bytes
            ));
        }
        assert!(encode(&views, &accesses, -1).is_err());
        if !views.is_empty() {
            let mut forged_views = views.clone();
            forged_views[0] = forged_view;
            assert!(encode(&forged_views, &accesses, 211).is_err());
        }
    }
}

/// Require every supervisor-call row to carry the kernel-entry effects, and
/// target-neutral admission to refuse each corruption of them without relying
/// on equality against the instruction set's own catalog. `other` is the
/// catalog of a target that has no kernel-entry rows.
pub fn effects_conformance(
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &MachineEffectCatalog,
    keys: &[RegisterConstraintKey],
    stack_pointer: RegisterViewId,
    other: &MachineEffectCatalog,
) {
    for &key in keys {
        let row = catalog
            .declarations
            .iter()
            .find(|row| row.constraint == key)
            .unwrap();
        assert_eq!(row.semantic, MachineSemanticKind::NormalizedForeignCall);
        assert_eq!(row.call, MachineCallEffect::SupervisorNormalReturnV1);
        let encoded = &row.alternatives[0].encoded;
        assert_eq!(
            encoded.control,
            MachineEncodedControlEffect::SupervisorCallV1
        );
        assert_eq!(encoded.memory, MachineEncodedMemoryEffect::NoneV1);
        assert_eq!(encoded.stack, MachineEncodedStackEffect::UnchangedV1);
        for corruption in 0..4 {
            let mut forged = catalog.clone();
            let row = forged
                .declarations
                .iter_mut()
                .find(|row| row.constraint == key)
                .unwrap();
            match corruption {
                0 => {
                    row.call = MachineCallEffect::ExternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    }
                }
                1 => {
                    row.alternatives[0].encoded.control =
                        MachineEncodedControlEffect::DirectRelativeCallV1
                }
                2 => {
                    row.alternatives[0].encoded.stack =
                        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                            stack_pointer,
                            return_address_byte_count: 8,
                        }
                }
                _ => row.alternatives[0].encoded.implicit_unit_clobbers.clear(),
            }
            assert!(validate_machine_effect_catalog(constraints, forged).is_err());
        }
    }
    assert!(
        other
            .selected_keys
            .call_normalized_foreign
            .iter()
            .all(|key| key.family != RegisterConstraintFamily::SystemCall)
    );
}
