//! Raw IEEE register transport. These instructions neither evaluate floats nor touch control state.
use super::*;
type Error = X86_64SelectedFormEncodingError;
type Encoding = ValidatedX86_64SelectedFormEncoding;
pub(super) fn is_transfer(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::Float32ToBits
            | SelectedInstructionKind::Float64ToBits
            | SelectedInstructionKind::BitsToFloat32
            | SelectedInstructionKind::BitsToFloat64
    )
}
fn properties(kind: SelectedInstructionKind) -> Option<(MachineAlternativeFamily, bool, bool)> {
    Some(match kind {
        SelectedInstructionKind::Float32ToBits => {
            (MachineAlternativeFamily::Float32ToBits, true, false)
        }
        SelectedInstructionKind::Float64ToBits => {
            (MachineAlternativeFamily::Float64ToBits, true, true)
        }
        SelectedInstructionKind::BitsToFloat32 => {
            (MachineAlternativeFamily::BitsToFloat32, false, false)
        }
        SelectedInstructionKind::BitsToFloat64 => {
            (MachineAlternativeFamily::BitsToFloat64, false, true)
        }
        _ => return None,
    })
}

fn operands(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    values: &[RegisterViewId],
) -> Result<(u8, u8, bool, bool), Error> {
    if physical.model() != &crate::x86_64_physical_register_model() {
        return Err(Error::NonCanonicalPhysicalModel);
    }
    let (family, to_bits, wide) = properties(kind).ok_or(Error::AlternativeMismatch)?;
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Error::AlternativeMismatch);
    }
    let [source, destination] = values else {
        return Err(Error::OperandCountMismatch);
    };
    let (float, bits) = if to_bits {
        (*source, *destination)
    } else {
        (*destination, *source)
    };
    let float_view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == float)
        .ok_or(Error::UnknownOrNonGpr64View(float))?;
    let float_register = float_view
        .name
        .strip_prefix("xmm")
        .and_then(|suffix| suffix.parse::<u8>().ok())
        .filter(|number| *number <= 15)
        .ok_or(Error::UnknownOrNonGpr64View(float))?;
    if float_view.bits != 128 || !float_view.allocatable {
        return Err(Error::UnknownOrNonGpr64View(float));
    }
    let bits_register = resolve_registers(physical, &[bits])?[0];
    Ok((float_register, bits_register, to_bits, wide))
}
pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    values: &[RegisterViewId],
) -> Result<Encoding, Error> {
    let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
    let mut bytes = vec![0x66];
    let rex = 0x40 | (u8::from(wide) << 3) | ((float >> 3) << 2) | (bits >> 3);
    if rex != 0x40 {
        bytes.push(rex);
    }
    bytes.extend([
        0x0f,
        if to_bits { 0x7e } else { 0x6e },
        0xc0 | ((float & 7) << 3) | (bits & 7),
    ]);
    validate(physical, kind, alternative, values, &bytes)
}
pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    values: &[RegisterViewId],
    bytes: &[u8],
) -> Result<Encoding, Error> {
    let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
    let [0x66, tail @ ..] = bytes else {
        return Err(Error::MalformedEncoding);
    };
    let (rex, tail) = match tail {
        [rex @ 0x41..=0x4f, rest @ ..] => (*rex, rest),
        _ => (0x40, tail),
    };
    let [0x0f, opcode, modrm] = tail else {
        return Err(Error::MalformedEncoding);
    };
    if modrm & 0xc0 != 0xc0 || rex & 2 != 0 {
        return Err(Error::MalformedEncoding);
    }
    let decoded_to_bits = match opcode {
        0x7e => true,
        0x6e => false,
        _ => return Err(Error::MalformedEncoding),
    };
    let decoded_float = ((modrm >> 3) & 7) | ((rex & 4) << 1);
    let decoded_bits = (modrm & 7) | ((rex & 1) << 3);
    if (decoded_float, decoded_bits, decoded_to_bits, rex & 8 != 0) != (float, bits, to_bits, wide)
    {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(Encoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: vec![values[0]],
            register_writes: vec![values[1]],
            writes_rflags: false,
            encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]),
        },
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_transfers_decode_exact_width_direction_and_registers() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        for (kind, family) in [
            (
                SelectedInstructionKind::Float32ToBits,
                MachineAlternativeFamily::Float32ToBits,
            ),
            (
                SelectedInstructionKind::Float64ToBits,
                MachineAlternativeFamily::Float64ToBits,
            ),
            (
                SelectedInstructionKind::BitsToFloat32,
                MachineAlternativeFamily::BitsToFloat32,
            ),
            (
                SelectedInstructionKind::BitsToFloat64,
                MachineAlternativeFamily::BitsToFloat64,
            ),
        ] {
            for float_name in ["xmm0", "xmm15"] {
                for bits_name in ["rax", "r15"] {
                    let float = physical.model().view_named(float_name).unwrap().id;
                    let bits = physical.model().view_named(bits_name).unwrap().id;
                    let values = if properties(kind).unwrap().1 {
                        [float, bits]
                    } else {
                        [bits, float]
                    };
                    let alternative = MachineAlternativeKey { family, variant: 0 };
                    let encoded = encode(&physical, kind, alternative, &values).unwrap();
                    assert_eq!(
                        encoded.footprint.encoded,
                        MachineEncodedEffects::fallthrough_v1(vec![0], vec![1])
                    );
                    for byte_index in 0..encoded.bytes.len() {
                        let mut changed = encoded.bytes.clone();
                        changed[byte_index] ^= 1;
                        assert!(validate(&physical, kind, alternative, &values, &changed).is_err());
                    }
                    assert!(encode(&physical, kind, alternative, &[values[1], values[0]]).is_err());
                }
            }
        }
    }
}
