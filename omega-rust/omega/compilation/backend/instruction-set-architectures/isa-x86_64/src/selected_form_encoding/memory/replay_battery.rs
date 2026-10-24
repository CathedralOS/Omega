//! One rejection battery for every memory form's independent replay.
//!
//! Each form re-derives its fields from the byte stream and compares them with
//! the request, so the same three stream mutations must be rejected everywhere:
//! a changed bit, a truncated prefix, and an appended byte. Spelling the sweep
//! once keeps every form's evidence the same strength; before this owner, each
//! test module asserted a different subset (bytes instead of bits, no truncated
//! prefixes, no appended byte), so the weakest copy defined what the replay
//! guarantee actually proved.
use super::{ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError};
use register_model::RegisterViewId;
use selected_instructions::{MachineAlternativeKey, SelectedInstructionKind};

/// Require `replay` to accept the exact stream and to reject every bit
/// corruption, every truncated prefix and one appended byte.
pub(super) fn replay_rejects_byte_mutations(
    encoded: &[u8],
    replay: impl Fn(
        &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError>,
) {
    assert!(replay(encoded).is_ok(), "the exact stream must replay");
    for bit in 0..encoded.len() * 8 {
        let mut changed = encoded.to_vec();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(
            replay(&changed).is_err(),
            "accepted changed encoding bit {bit}"
        );
    }
    for end in 0..encoded.len() {
        assert!(
            replay(&encoded[..end]).is_err(),
            "accepted truncated prefix {end}"
        );
    }
    let mut extended = encoded.to_vec();
    extended.push(0);
    assert!(replay(&extended).is_err(), "accepted an appended byte");
}

/// Require the same stream to be rejected for a substituted request: a variant
/// the form does not declare, a neighbouring displacement, and a shorter
/// operand list.
pub(super) fn replay_rejects_substituted_requests(
    encoded: &[u8],
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    replay: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId],
        i64,
        &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError>,
) {
    let undeclared = MachineAlternativeKey {
        family: alternative.family,
        variant: alternative.variant + 1,
    };
    assert!(
        replay(kind, undeclared, operands, displacement, encoded).is_err(),
        "accepted an undeclared alternative variant"
    );
    assert!(
        replay(kind, alternative, operands, displacement + 1, encoded).is_err(),
        "accepted a neighbouring displacement"
    );
    let shorter = &operands[..operands.len().saturating_sub(1)];
    assert!(
        replay(kind, alternative, shorter, displacement, encoded).is_err(),
        "accepted a shorter operand list"
    );
}
