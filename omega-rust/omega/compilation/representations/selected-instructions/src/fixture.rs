//! The refusal records a selected form's encoder states about its own bytes.
//!
//! An encoder and the validator that re-derives its bytes are defined as much
//! by what they refuse as by what they emit. That list — a mutated word, a
//! truncated or extended stream, a foreign operand, another alternative
//! family, altered effect custody — is one record between the instruction
//! sets; only the admitted bytes differ (`ud2` against `brk #0`, `call rel32`
//! against `bl`). Each set supplies its encoder, its word, and the views its
//! physical model pins, and the record that reads them is stated here once.

use crate::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
};
use register_model::{
    RegisterInstructionConstraint, RegisterViewId, ValidatedPhysicalRegisterModel,
};
use semantic_vocabulary::MachineId;

/// A validated selected-form encoding as the byte record reads it.
pub trait EncodedWord {
    fn encoded_bytes(&self) -> &[u8];
    fn encoded_effects(&self) -> &MachineEncodedEffects;
}

/// A validated ordinary-call template as the mixed-row record reads it.
pub trait CallTemplate {
    fn bound_operands(&self) -> &[RegisterViewId];
    /// The template's unresolved fixup is the canonical one for `callee`.
    fn names_callee(&self, callee: MachineId) -> bool;
}

/// The exact word one instruction set's crash leaf encodes.
pub struct CrashWord<'a> {
    pub physical: &'a ValidatedPhysicalRegisterModel,
    pub bytes: &'a [u8],
    /// The program-counter view the trap records as used and defined.
    pub program_counter: &'a str,
    /// A view the operand-free leaf must refuse as an operand.
    pub foreign_operand: &'a str,
}

/// Require the crash leaf to encode `word` exactly and to refuse every mutation
/// of it. The word's length fixes the bit-flip count and the truncated stream,
/// so a two-byte and a four-byte instruction set state the same record.
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

/// Require one mixed unit-call row to encode a template that binds exactly its
/// fixed operand views, and to refuse every request its ABI cannot produce.
/// `base` is the instruction set's ordinary-call effect row for the target;
/// the row, its alternative, and its callee fixup are the same in both.
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
