//! Independently decode the carried legal branch alternatives.

use crate::OptimizedSelectedFormEncodingError as Error;
use machine_code::{
    BranchDisplacementField as Field, EncodedBranchAlternative, EncodedControlAlternatives,
    SelectedFormDecodedFootprint,
};
use post_allocation_machine::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{MachineAlternativeFamily as Family, SelectedInstructionKind as Kind};
use target::{Architecture, NativeTarget};

macro_rules! alternative {
    ($encoded:expr, $key:expr, $field:expr, $offset:expr, $reference:expr) => {{
        let encoded = $encoded;
        let footprint = encoded.footprint();
        EncodedBranchAlternative {
            alternative: $key,
            bytes: encoded.bytes().to_vec(),
            field: $field,
            field_offset: $offset,
            reference_offset: $reference,
            footprint: SelectedFormDecodedFootprint {
                register_reads: footprint.register_reads.clone(),
                register_writes: footprint.register_writes.clone(),
                implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
                implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
                encoded: footprint.encoded.clone(),
            },
        }
    }};
}

/// Decode the supplied forms independently; do not regenerate their bytes.
pub(crate) fn validate(
    target: NativeTarget,
    kind: Kind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    forms: &EncodedControlAlternatives,
) -> Result<(), Error> {
    let jump = kind == Kind::Jump;
    if forms.widened.is_some() != (target.architecture == Architecture::Aarch64 && !jump)
        || forms.shortened.is_some() != (target.architecture == Architecture::X86_64 && !jump)
    {
        return Err(Error::ArtifactMismatch);
    }
    for (form, row) in std::iter::once((0, &forms.baseline))
        .chain(forms.widened.iter().map(|row| (1, row)))
        .chain(forms.shortened.iter().map(|row| (2, row)))
    {
        let mut key = machine.alternative.key;
        if form == 2 && kind != Kind::ConditionalBranchNonZero {
            key.variant = 1;
        }
        let expected = match target.architecture {
            Architecture::X86_64 => {
                let decoded = match kind {
                    Kind::Jump => {
                        isa_x86_64::validate_x86_64_selected_jump_form(physical, key, 0, &row.bytes)
                    }
                    Kind::ConditionalBranchNonZero if form == 2 => {
                        isa_x86_64::validate_x86_64_selected_short_nonzero_branch_form(
                            physical, key, 0, &row.bytes,
                        )
                    }
                    Kind::ConditionalBranchNonZero => {
                        isa_x86_64::validate_x86_64_selected_nonzero_branch_form(
                            physical, key, 0, &row.bytes,
                        )
                    }
                    Kind::ConditionalBranchU64LessThan => {
                        isa_x86_64::validate_x86_64_selected_u64_less_than_branch_form(
                            physical, key, 0, &row.bytes,
                        )
                    }
                    Kind::ConditionalBranchI64LessThan => {
                        isa_x86_64::validate_x86_64_selected_i64_less_than_branch_form(
                            physical, key, 0, &row.bytes,
                        )
                    }
                    _ => return Err(Error::ArtifactMismatch),
                }
                .map_err(Error::X86_64)?;
                if form == 2 {
                    alternative!(decoded, key, Field::Relative8, 1, 2)
                } else {
                    alternative!(
                        decoded,
                        key,
                        Field::Relative32,
                        if jump { 1 } else { 2 },
                        if jump { 5 } else { 6 }
                    )
                }
            }
            Architecture::Aarch64 => {
                if jump {
                    alternative!(
                        isa_aarch64::validate_aarch64_selected_jump_form(
                            physical, key, 0, &row.bytes
                        )
                        .map_err(Error::Aarch64)?,
                        key,
                        Field::Aarch64Branch26,
                        0,
                        0
                    )
                } else {
                    let family = match kind {
                        Kind::ConditionalBranchNonZero => Family::ConditionalBranchNonZero,
                        Kind::ConditionalBranchU64LessThan => Family::ConditionalBranchU64LessThan,
                        Kind::ConditionalBranchI64LessThan => Family::ConditionalBranchI64LessThan,
                        _ => return Err(Error::ArtifactMismatch),
                    };
                    let decoded = isa_aarch64::validate_aarch64_selected_conditional_branch_form(
                        physical,
                        key,
                        family,
                        if form == 1 {
                            isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1
                        } else {
                            isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1
                        },
                        if form == 1 { 4 } else { 0 },
                        &row.bytes,
                    )
                    .map_err(Error::Aarch64)?;
                    if form == 1 {
                        alternative!(decoded, key, Field::Aarch64Branch26, 4, 4)
                    } else {
                        alternative!(decoded, key, Field::Aarch64Conditional19, 0, 0)
                    }
                }
            }
        };
        if *row != expected || row.footprint.encoded != machine.alternative.encoded {
            return Err(Error::ArtifactMismatch);
        }
    }
    Ok(())
}
