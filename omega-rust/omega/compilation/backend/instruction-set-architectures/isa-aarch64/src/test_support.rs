//! Shared sweeps for the encoder tests: an emitted form pins its whole word
//! stream, so a single changed bit must be refused by the form's validator.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{MachineAlternativeKey, SelectedInstructionKind};

/// Assert that the selected-form entrance refuses every single-bit change to
/// `bytes`: the form writes every field of every word it emits, so a corrupted
/// stream either fails to decode or mismatches the emitted plan.
pub(crate) fn rejects_every_bit_flip(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) {
    reject_every_bit_flip(bytes, &format!("{kind:?}"), |changed| {
        crate::validate_aarch64_selected_form_encoding(
            physical,
            kind,
            alternative,
            operands,
            changed,
        )
        .is_err()
    });
}

/// The same sweep for a form family whose validator is not the selected-form
/// entrance: `reject` reports whether the changed stream was refused.
pub(crate) fn reject_every_bit_flip(
    bytes: &[u8],
    description: &str,
    reject: impl Fn(&[u8]) -> bool,
) {
    for bit in 0..bytes.len() * 8 {
        let mut changed = bytes.to_vec();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(reject(&changed), "{description} accepted changed bit {bit}");
    }
}
