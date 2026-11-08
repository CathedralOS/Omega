/*
  Shared refusal and call-template test assertions for target instruction
  encoders (`x86_64` and `aarch64`).

  `assert_crash_refusals` verifies that a target's crash leaf encodes the exact
  expected bytes (`ud2` or `brk #0`) and rejects bit flips, truncated or
  extended byte streams, foreign operands, and wrong alternative keys.
  `assert_mixed_unit_call_row` verifies that a mixed ABI unit-call row binds
  its pinned physical operand views and callee fixup while rejecting any
  deviating operand or effect request.
*/

use crate::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
};
use register_model::{
    RegisterInstructionConstraint, RegisterViewId, ValidatedPhysicalRegisterModel,
};
use semantic_vocabulary::MachineId;

pub trait EncodedWord {
    fn encoded_bytes(&self) -> &[u8];
    fn encoded_effects(&self) -> &MachineEncodedEffects;
}

pub trait CallTemplate {
    fn bound_operands(&self) -> &[RegisterViewId];
    fn names_callee(&self, callee: MachineId) -> bool;
}

pub struct CrashWord<'a> {
    pub physical: &'a ValidatedPhysicalRegisterModel,
    pub bytes: &'a [u8],
    pub program_counter: &'a str, // The program-counter view the trap records as used and defined.
    pub foreign_operand: &'a str, // A view the operand-free leaf must refuse as an operand.
}

pub fn assert_crash_refusals<E, X>(
    word: CrashWord<'_>,
    alternative: MachineAlternativeKey,
    encode: impl Fn(SelectedInstructionKind, MachineAlternativeKey, &[RegisterViewId]) -> Result<E, X>,
    validate: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId],
        &[u8],
    ) -> Result<E, X>,
) where
    E: EncodedWord,
    X: std::fmt::Debug,
{
    let crash = SelectedInstructionKind::Crash;
    let encoded = encode(crash, alternative, &[]).unwrap();
    assert_eq!(encoded.encoded_bytes(), word.bytes);
    let program_counter = word
        .physical
        .model()
        .view_named(word.program_counter)
        .unwrap()
        .units
        .clone();
    assert_eq!(
        encoded.encoded_effects(),
        &MachineEncodedEffects::crash_v1(&program_counter)
    );
    for bit in 0..word.bytes.len() * 8 {
        let mut mutated = word.bytes.to_vec();
        mutated[bit / 8] ^= 1 << (bit % 8);
        assert!(validate(crash, alternative, &[], &mutated).is_err());
    }
    let mut extended = word.bytes.to_vec();
    extended.push(0);
    for bytes in [&extended[..], &word.bytes[..word.bytes.len() - 1]] {
        assert!(validate(crash, alternative, &[], bytes).is_err());
    }
    let foreign = [word
        .physical
        .model()
        .view_named(word.foreign_operand)
        .unwrap()
        .id];
    assert!(encode(crash, alternative, &foreign).is_err());
    for refused in [
        MachineAlternativeKey {
            variant: 1,
            ..alternative
        },
        MachineAlternativeKey {
            family: MachineAlternativeFamily::ReturnUnit,
            variant: 0,
        },
    ] {
        assert!(encode(crash, refused, &[]).is_err());
    }
    assert!(
        validate(
            SelectedInstructionKind::ReturnUnit,
            alternative,
            &[],
            word.bytes
        )
        .is_err()
    );
}

pub fn assert_mixed_unit_call_row<E, X>(
    row: &RegisterInstructionConstraint,
    base: MachineEncodedEffects,
    callee: MachineId,
    stack_pointer: RegisterViewId,
    encode: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId],
        &MachineEncodedEffects,
    ) -> Result<E, X>,
) where
    E: CallTemplate,
    X: std::fmt::Debug,
{
    let kind = SelectedInstructionKind::CallUnit {
        target: crate::SelectedCallTarget::Machine(callee),
    };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CallUnit,
        variant: 0,
    };
    let operands = row
        .operands
        .iter()
        .map(|operand| operand.fixed_view.expect("a mixed row pins every operand"))
        .collect::<Vec<_>>();
    let mut effects = base;
    effects.external_operand_reads = (0..operands.len() as u16).collect();
    effects.external_operand_writes.clear();
    effects.implicit_unit_uses = row.implicit_uses.clone();
    effects.implicit_unit_defs = row.implicit_defs.clone();
    effects.implicit_unit_clobbers = row.clobbers.clone();
    let template = encode(kind, alternative, &operands, &effects).expect("mixed ABI call template");
    assert_eq!(template.bound_operands(), operands);
    assert!(template.names_callee(callee));
    let mut wrong = operands.clone();
    wrong.push(operands[0]);
    assert!(encode(kind, alternative, &wrong, &effects).is_err());
    let mut wrong = operands.clone();
    wrong[0] = stack_pointer;
    assert!(encode(kind, alternative, &wrong, &effects).is_err());
    assert!(encode(kind, alternative, &operands[..operands.len() - 1], &effects).is_err());
    if operands.len() > 1 {
        let mut wrong = operands.clone();
        wrong.swap(0, operands.len() - 1);
        assert!(encode(kind, alternative, &wrong, &effects).is_err());
    }
    let mut wrong = effects.clone();
    wrong.implicit_unit_clobbers.clear();
    assert!(encode(kind, alternative, &operands, &wrong).is_err());
    let mut wrong = effects.clone();
    wrong.external_operand_writes.push(0);
    assert!(encode(kind, alternative, &operands, &wrong).is_err());
}
