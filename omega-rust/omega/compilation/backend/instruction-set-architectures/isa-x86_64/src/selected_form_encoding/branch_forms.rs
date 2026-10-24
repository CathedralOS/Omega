//! Encoding and validating the nonzero, short nonzero, unsigned and
//! signed less-than branch forms.
//!
//! Every family realizes the same two x86-64 widths and differs only in its
//! condition code: `Jcc rel8` is opcode `70+cc` and `Jcc rel32` opcode
//! `0F 80+cc`, both with the displacement measured from the instruction end.
//! One row per family names its condition code, its selected kind and the
//! variants it selects; one request check, one byte builder and one decoder
//! read that row, so a family's bytes and its displacement check exist once.
//! Each entrypoint names its own family, so a form still refuses an
//! alternative that selects another family's encoding.

use crate::selected_form_encoding::decoding::footprint;
use crate::selected_form_encoding::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
};

/// One realized conditional branch family.
#[derive(Clone, Copy)]
struct BranchFamily {
    /// The alternative family this row realizes.
    family: MachineAlternativeFamily,
    /// The selected kind whose footprint a validated form carries.
    kind: SelectedInstructionKind,
    /// The x86-64 condition code of the family's mnemonic.
    condition: u8,
    /// The alternatives the family selects: one for
    /// `ConditionalBranchNonZero`, whose short form has a dedicated entrypoint
    /// at variant zero, and two for the less-than families, whose alternative
    /// carries the short width as variant one.
    variant_count: u32,
}

/// `Jcc rel8`: the condition code in the low nibble of `0x70`.
const fn short_opcode(condition: u8) -> u8 {
    0x70 | condition
}

/// `Jcc rel32`: the condition code in the low nibble of `0x80`, behind the
/// two-byte opcode escape `0x0F`.
const fn near_opcode(condition: u8) -> [u8; 2] {
    [0x0f, 0x80 | condition]
}

/// JNE / JNZ.
const NONZERO: BranchFamily = BranchFamily {
    family: MachineAlternativeFamily::ConditionalBranchNonZero,
    kind: SelectedInstructionKind::ConditionalBranchNonZero,
    condition: 0x5,
    variant_count: 1,
};

/// JB / JNAE / JC.
const U64_LESS_THAN: BranchFamily = BranchFamily {
    family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
    kind: SelectedInstructionKind::ConditionalBranchU64LessThan,
    condition: 0x2,
    variant_count: 2,
};

/// JL / JNGE.
const I64_LESS_THAN: BranchFamily = BranchFamily {
    family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
    kind: SelectedInstructionKind::ConditionalBranchI64LessThan,
    condition: 0xc,
    variant_count: 2,
};

/// How an entrypoint selects the width it encodes: the two dedicated
/// entrypoints name it, and a less-than family carries it on its
/// alternative's variant.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WidthRule {
    Near,
    Short,
    Variant,
}

/// One checked branch request: the family row its entrypoint names, the width
/// that entrypoint or the alternative selected, and the alternative the
/// footprint records. Construction performs the whole request check - canonical
/// model, the entrypoint's own family, and the variants that family selects -
/// so no entrypoint repeats it.
#[derive(Clone, Copy)]
struct BranchRequest {
    row: BranchFamily,
    alternative: MachineAlternativeKey,
    near: bool,
}

impl BranchRequest {
    fn new(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        row: BranchFamily,
        rule: WidthRule,
    ) -> Result<Self, X86_64SelectedFormEncodingError> {
        super::request_validation::guard_canonical(physical)?;
        if alternative.family != row.family || alternative.variant >= row.variant_count {
            return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
        }
        let near = match rule {
            WidthRule::Near => true,
            WidthRule::Short => false,
            WidthRule::Variant => alternative.variant == 0,
        };
        Ok(Self {
            row,
            alternative,
            near,
        })
    }

    /// Encode this request: the bytes its width and displacement name, then
    /// the same check a candidate encoding gets.
    fn encode(
        self,
        byte_displacement_from_instruction_end: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let bytes = self.bytes(byte_displacement_from_instruction_end)?;
        self.validate(byte_displacement_from_instruction_end, &bytes)
    }

    /// Validate one candidate encoding against this request.
    fn validate(
        self,
        byte_displacement_from_instruction_end: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        self.validate_bytes(byte_displacement_from_instruction_end, bytes)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(self.row.kind, self.alternative, &[]),
        })
    }

    /// The canonical bytes of this request: the near form carries the signed
    /// 32-bit displacement in its last four bytes, the short form the signed
    /// 8-bit displacement in its last byte.
    fn bytes(
        self,
        byte_displacement_from_instruction_end: i64,
    ) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
        if self.near {
            let displacement = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let mut bytes = near_opcode(self.row.condition).to_vec();
            bytes.extend(displacement.to_le_bytes());
            return Ok(bytes);
        }
        let displacement = i8::try_from(byte_displacement_from_instruction_end)
            .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
        Ok(vec![short_opcode(self.row.condition), displacement as u8])
    }

    /// Independently decode exactly one canonical instruction of this request's
    /// width: the exact opcode, that width and nothing more, and the
    /// displacement the request names. The sibling width, a prefix, truncation
    /// and extension are not alternate encodings of a selected branch form.
    fn validate_bytes(
        self,
        byte_displacement_from_instruction_end: i64,
        bytes: &[u8],
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        let (expected, encoded) = if self.near {
            let expected = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let encoded = bytes
                .get(2..6)
                .filter(|_| bytes.len() == 6 && bytes[..2] == near_opcode(self.row.condition))
                .and_then(|displacement| displacement.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            (i64::from(expected), i64::from(encoded))
        } else {
            let expected = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            let encoded = bytes
                .get(1)
                .copied()
                .filter(|_| bytes.len() == 2 && bytes[0] == short_opcode(self.row.condition))
                .map(|displacement| displacement as i8)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            (i64::from(expected), i64::from(encoded))
        };
        if expected != encoded {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(())
    }
}

/// Encode the canonical layout-resolved realization of
/// `ConditionalBranchNonZero`: the six-byte near `JNE rel32`.
pub fn encode_x86_64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Near)?;
    request.encode(byte_displacement_from_instruction_end)
}

/// Independently decode exactly one canonical `JNE rel32` instruction.
pub fn validate_x86_64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Near)?;
    request.validate(byte_displacement_from_instruction_end, bytes)
}

/// Encode the canonical short layout-resolved realization of
/// `ConditionalBranchNonZero`: the two-byte `JNE rel8`.
pub fn encode_x86_64_selected_short_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Short)?;
    request.encode(byte_displacement_from_instruction_end)
}

/// Validate exactly one canonical x86-64 `JNE rel8` instruction. Near-branch
/// opcodes, prefixes, suffixes, and trailing bytes are not alternate encodings
/// of this selected short form.
pub fn validate_x86_64_selected_short_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Short)?;
    request.validate(byte_displacement_from_instruction_end, bytes)
}

/// Encode the selected layout-resolved unsigned-less-than branch: alternative
/// zero the six-byte `JB rel32`, alternative one the two-byte `JB rel8`.
pub fn encode_x86_64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, U64_LESS_THAN, WidthRule::Variant)?;
    request.encode(byte_displacement_from_instruction_end)
}

/// Independently decode the exact `JB` form named by the alternative.
pub fn validate_x86_64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, U64_LESS_THAN, WidthRule::Variant)?;
    request.validate(byte_displacement_from_instruction_end, bytes)
}

/// Encode the selected layout-resolved signed-less-than branch: alternative
/// zero the six-byte `JL rel32`, alternative one the two-byte `JL rel8`.
pub fn encode_x86_64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, I64_LESS_THAN, WidthRule::Variant)?;
    request.encode(byte_displacement_from_instruction_end)
}

/// Independently decode the exact `JL` form named by the alternative.
pub fn validate_x86_64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let request = BranchRequest::new(physical, alternative, I64_LESS_THAN, WidthRule::Variant)?;
    request.validate(byte_displacement_from_instruction_end, bytes)
}
