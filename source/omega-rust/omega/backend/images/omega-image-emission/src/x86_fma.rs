use iced_x86::{Decoder, DecoderOptions, Mnemonic};
use omega_calling_conventions::MachineRegister;
use omega_machine_code::{MachineCodeFunction, X86ScalarFmaFormat};
use omega_target::{Architecture, NativeTarget, TargetProfile};

use crate::ObjectError;

pub(crate) fn validate_x86_scalar_fma_function(
    target: NativeTarget,
    profile: Option<TargetProfile>,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    if target.architecture != Architecture::X86_64 {
        if function.x86_scalar_fma.is_empty() {
            return Ok(());
        }
        return Err(ObjectError::X86ScalarFmaUnsupportedTarget(function.machine));
    }

    let mut decoded_offsets = Vec::new();
    let mut decoder = Decoder::with_ip(64, &function.bytes, 0, DecoderOptions::NONE);
    while decoder.can_decode() {
        let instruction = decoder.decode();
        if matches!(
            instruction.mnemonic(),
            Mnemonic::Vfmadd132ss | Mnemonic::Vfmadd132sd
        ) {
            decoded_offsets.push(instruction.ip() as usize);
        }
    }
    if function.x86_scalar_fma.is_empty() {
        return decoded_offsets.first().copied().map_or(Ok(()), |offset| {
            Err(ObjectError::MissingX86ScalarFmaCustody {
                machine: function.machine,
                offset,
            })
        });
    }
    let profile = profile.ok_or(ObjectError::MissingX86ScalarFmaProfile(function.machine))?;

    let mut previous_end = None;
    for fragment in &function.x86_scalar_fma {
        if let Some(end) = previous_end {
            if fragment.code_offset < end {
                return Err(ObjectError::NonCanonicalX86ScalarFmaOrder(function.machine));
            }
        }
        if fragment.byte_count != 5 {
            return Err(ObjectError::InvalidX86ScalarFmaInterval {
                machine: function.machine,
                offset: fragment.code_offset,
            });
        }
        let end = fragment
            .code_offset
            .checked_add(fragment.byte_count)
            .ok_or(ObjectError::InvalidX86ScalarFmaInterval {
                machine: function.machine,
                offset: fragment.code_offset,
            })?;
        let bytes = function.bytes.get(fragment.code_offset..end).ok_or(
            ObjectError::InvalidX86ScalarFmaInterval {
                machine: function.machine,
                offset: fragment.code_offset,
            },
        )?;
        if fragment.target != target
            || fragment.requirement.profile() != profile
            || fragment.requirement.profile().native_target() != target
            || !fragment.requirement.has_canonical_identity()
            || fragment.recomputed_identity() != Some(fragment.identity)
        {
            return Err(ObjectError::InvalidX86ScalarFmaCustody {
                machine: function.machine,
                offset: fragment.code_offset,
            });
        }
        let decoded = omega_isa_x86_64::decode_vfmadd132_scalar(bytes).map_err(|_| {
            ObjectError::InvalidX86ScalarFmaEncoding {
                machine: function.machine,
                offset: fragment.code_offset,
            }
        })?;
        let format = match decoded.format {
            omega_isa_x86_64::DecodedScalarFmaFormat::Binary32 => X86ScalarFmaFormat::Binary32,
            omega_isa_x86_64::DecodedScalarFmaFormat::Binary64 => X86ScalarFmaFormat::Binary64,
        };
        if format != fragment.format
            || decoded.destination != fragment.destination
            || decoded.addend != fragment.addend
            || decoded.multiplicand != fragment.multiplicand
            || !matches!(fragment.destination, MachineRegister::X86Xmm(0..=15))
            || !matches!(fragment.addend, MachineRegister::X86Xmm(0..=15))
            || !matches!(fragment.multiplicand, MachineRegister::X86Xmm(0..=15))
        {
            return Err(ObjectError::InvalidX86ScalarFmaCustody {
                machine: function.machine,
                offset: fragment.code_offset,
            });
        }
        previous_end = Some(end);
    }

    let retained_offsets = function
        .x86_scalar_fma
        .iter()
        .map(|fragment| fragment.code_offset)
        .collect::<Vec<_>>();
    if decoded_offsets != retained_offsets {
        let offset = decoded_offsets
            .iter()
            .find(|offset| !retained_offsets.contains(offset))
            .copied()
            .or_else(|| retained_offsets.first().copied())
            .unwrap_or(0);
        return Err(ObjectError::MissingX86ScalarFmaCustody {
            machine: function.machine,
            offset,
        });
    }
    Ok(())
}
