//! Encode all legal branch forms before layout knows their displacements.

use crate::OptimizedSelectedFormEncodingError as Error;
use machine_code::{
    BranchDisplacementField as Field, EncodedBranchAlternative, EncodedControlAlternatives,
    SelectedFormDecodedFootprint,
};
use post_allocation_machine::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineAlternativeFamily as Family, MachineAlternativeKey, SelectedInstructionKind as Kind,
};
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

pub(crate) fn encode(
    target: NativeTarget,
    kind: Kind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<EncodedControlAlternatives, Error> {
    let key = machine.alternative.key;
    let family = match kind {
        Kind::Jump => Family::Jump,
        Kind::ConditionalBranchNonZero => Family::ConditionalBranchNonZero,
        Kind::ConditionalBranchU64LessThan => Family::ConditionalBranchU64LessThan,
        Kind::ConditionalBranchI64LessThan => Family::ConditionalBranchI64LessThan,
        _ => return Err(Error::ArtifactMismatch),
    };
    if key.family != family {
        return Err(Error::ArtifactMismatch);
    }
    let result = match target.architecture {
        Architecture::X86_64 => {
            let encode = |key: MachineAlternativeKey, short: bool| {
                match kind {
                    Kind::Jump => isa_x86_64::encode_x86_64_selected_jump_form(physical, key, 0),
                    Kind::ConditionalBranchNonZero if short => {
                        isa_x86_64::encode_x86_64_selected_short_nonzero_branch_form(
                            physical, key, 0,
                        )
                    }
                    Kind::ConditionalBranchNonZero => {
                        isa_x86_64::encode_x86_64_selected_nonzero_branch_form(physical, key, 0)
                    }
                    Kind::ConditionalBranchU64LessThan => {
                        isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(
                            physical, key, 0,
                        )
                    }
                    Kind::ConditionalBranchI64LessThan => {
                        isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(
                            physical, key, 0,
                        )
                    }
                    _ => unreachable!("the control family was checked"),
                }
                .map_err(Error::X86_64)
            };
            let jump = kind == Kind::Jump;
            let baseline = alternative!(
                encode(key, false)?,
                key,
                Field::Relative32,
                if jump { 1 } else { 2 },
                if jump { 5 } else { 6 }
            );
            let shortened = if jump {
                None
            } else {
                let short_key = MachineAlternativeKey {
                    variant: if kind == Kind::ConditionalBranchNonZero {
                        key.variant
                    } else {
                        1
                    },
                    ..key
                };
                Some(alternative!(
                    encode(short_key, true)?,
                    short_key,
                    Field::Relative8,
                    1,
                    2
                ))
            };
            EncodedControlAlternatives {
                baseline,
                widened: None,
                shortened,
            }
        }
        Architecture::Aarch64 => {
            if kind == Kind::Jump {
                EncodedControlAlternatives {
                    baseline: alternative!(
                        isa_aarch64::encode_aarch64_selected_jump_form(physical, key, 0)
                            .map_err(Error::Aarch64)?,
                        key,
                        Field::Aarch64Branch26,
                        0,
                        0
                    ),
                    widened: None,
                    shortened: None,
                }
            } else {
                let baseline = isa_aarch64::encode_aarch64_selected_conditional_branch_form(
                    physical,
                    key,
                    family,
                    isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1,
                    0,
                )
                .map_err(Error::Aarch64)?;
                // The second instruction branches relative to itself, four
                // bytes after the source conditional. Its template field is zero.
                let widened = isa_aarch64::encode_aarch64_selected_conditional_branch_form(
                    physical,
                    key,
                    family,
                    isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1,
                    4,
                )
                .map_err(Error::Aarch64)?;
                EncodedControlAlternatives {
                    baseline: alternative!(baseline, key, Field::Aarch64Conditional19, 0, 0),
                    widened: Some(alternative!(widened, key, Field::Aarch64Branch26, 4, 4)),
                    shortened: None,
                }
            }
        }
    };
    if std::iter::once(&result.baseline)
        .chain(result.widened.iter())
        .chain(result.shortened.iter())
        .any(|row| row.footprint.encoded != machine.alternative.encoded)
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(result)
}
