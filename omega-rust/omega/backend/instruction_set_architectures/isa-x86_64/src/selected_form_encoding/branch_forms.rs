//! Encoding and validating the nonzero, short nonzero, unsigned and
//! signed less-than branch forms.

use crate::selected_form_encoding::decoding::footprint;
use crate::selected_form_encoding::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
};

/// Encode the canonical layout-resolved realization of
/// `ConditionalBranchNonZero`. The displacement is measured from the end of
/// this six-byte near branch, as required by x86-64.
pub fn encode_x86_64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let displacement = i32::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
    let mut bytes = vec![0x0f, 0x85];
    bytes.extend(displacement.to_le_bytes());
    validate_x86_64_selected_nonzero_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &bytes,
    )
}

pub fn validate_x86_64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let expected = i32::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
    let actual = bytes
        .get(2..6)
        .filter(|_| bytes.len() == 6 && bytes[..2] == [0x0f, 0x85])
        .and_then(|bytes| bytes.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if actual != expected {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchNonZero,
            alternative,
            &[],
        ),
    })
}

/// Encode the canonical short layout-resolved realization of
/// `ConditionalBranchNonZero`. The signed byte displacement is measured from
/// the end of this two-byte instruction, as required by x86-64 `JNE rel8`.
pub fn encode_x86_64_selected_short_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let displacement = i8::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
    validate_x86_64_selected_short_nonzero_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &[0x75, displacement as u8],
    )
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
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let expected = i8::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
    let actual = bytes
        .get(1)
        .copied()
        .filter(|_| bytes.len() == 2 && bytes[0] == 0x75)
        .map(|displacement| displacement as i8)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if actual != expected {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchNonZero,
            alternative,
            &[],
        ),
    })
}

/// Encode the selected layout-resolved unsigned-less-than branch. Alternative
/// zero is the six-byte `JB rel32`; alternative one is the two-byte `JB rel8`.
/// In both cases the displacement is measured from the instruction end.
pub fn encode_x86_64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_less_than_branch_request(physical, alternative)?;
    let bytes = match alternative.variant {
        0 => {
            let displacement = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let mut bytes = vec![0x0f, 0x82];
            bytes.extend(displacement.to_le_bytes());
            bytes
        }
        1 => {
            let displacement = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            vec![0x72, displacement as u8]
        }
        _ => unreachable!("less-than branch request validated variant"),
    };
    validate_x86_64_selected_u64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &bytes,
    )
}

/// Independently decode the exact `JB` form named by the alternative.
pub fn validate_x86_64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_less_than_branch_request(physical, alternative)?;
    match alternative.variant {
        0 => {
            let expected = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let actual = bytes
                .get(2..6)
                .filter(|_| bytes.len() == 6 && bytes[..2] == [0x0f, 0x82])
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        1 => {
            let expected = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            let actual = bytes
                .get(1)
                .copied()
                .filter(|_| bytes.len() == 2 && bytes[0] == 0x72)
                .map(|displacement| displacement as i8)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        _ => unreachable!("less-than branch request validated variant"),
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchU64LessThan,
            alternative,
            &[],
        ),
    })
}

/// Encode the selected layout-resolved signed-less-than branch. Alternative
/// zero is the six-byte `JL rel32`; alternative one is the two-byte `JL rel8`.
/// In both cases the displacement is measured from the instruction end.
pub fn encode_x86_64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_i64_less_than_branch_request(physical, alternative)?;
    let bytes = match alternative.variant {
        0 => {
            let displacement = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let mut bytes = vec![0x0f, 0x8c];
            bytes.extend(displacement.to_le_bytes());
            bytes
        }
        1 => {
            let displacement = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            vec![0x7c, displacement as u8]
        }
        _ => unreachable!("signed less-than branch request validated variant"),
    };
    validate_x86_64_selected_i64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &bytes,
    )
}

/// Independently decode the exact `JL` form named by the alternative.
pub fn validate_x86_64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_i64_less_than_branch_request(physical, alternative)?;
    match alternative.variant {
        0 => {
            let expected = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let actual = bytes
                .get(2..6)
                .filter(|_| bytes.len() == 6 && bytes[..2] == [0x0f, 0x8c])
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        1 => {
            let expected = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            let actual = bytes
                .get(1)
                .copied()
                .filter(|_| bytes.len() == 2 && bytes[0] == 0x7c)
                .map(|displacement| displacement as i8)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        _ => unreachable!("signed less-than branch request validated variant"),
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchI64LessThan,
            alternative,
            &[],
        ),
    })
}

fn validate_i64_less_than_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative.family != MachineAlternativeFamily::ConditionalBranchI64LessThan
        || alternative.variant > 1
    {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn validate_less_than_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative.family != MachineAlternativeFamily::ConditionalBranchU64LessThan
        || alternative.variant > 1
    {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn validate_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}
