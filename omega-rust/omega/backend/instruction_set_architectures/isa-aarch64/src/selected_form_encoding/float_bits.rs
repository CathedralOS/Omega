//! Raw IEEE register transport. These instructions neither evaluate floats nor touch control state.
use super::*;
type Error = Aarch64SelectedFormEncodingError;
type Encoding = ValidatedAarch64SelectedFormEncoding;
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
    if physical.model() != &crate::aarch64_physical_register_model() {
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
        .strip_prefix("d")
        .and_then(|suffix| suffix.parse::<u8>().ok())
        .filter(|number| *number <= 31)
        .ok_or(Error::UnknownOrNonGpr64View(float))?;
    if float_view.bits != 64 || !float_view.allocatable {
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
    let opcode = match (to_bits, wide) {
        (true, false) => 0x1e26_0000_u32,
        (true, true) => 0x9e66_0000,
        (false, false) => 0x1e27_0000,
        (false, true) => 0x9e67_0000,
    };
    let (source, destination) = if to_bits {
        (float, bits)
    } else {
        (bits, float)
    };
    let bytes = (opcode | (u32::from(source) << 5) | u32::from(destination))
        .to_le_bytes()
        .to_vec();
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
    let raw: [u8; 4] = bytes.try_into().map_err(|_| Error::MalformedEncoding)?;
    let word = u32::from_le_bytes(raw);
    let (decoded_to_bits, decoded_wide) = match word & 0xffff_fc00 {
        0x1e26_0000 => (true, false),
        0x9e66_0000 => (true, true),
        0x1e27_0000 => (false, false),
        0x9e67_0000 => (false, true),
        _ => return Err(Error::MalformedEncoding),
    };
    let source = ((word >> 5) & 31) as u8;
    let destination = (word & 31) as u8;
    let (decoded_float, decoded_bits) = if decoded_to_bits {
        (source, destination)
    } else {
        (destination, source)
    };
    if (decoded_float, decoded_bits, decoded_to_bits, decoded_wide) != (float, bits, to_bits, wide)
    {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(Encoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: vec![values[0]],
            register_writes: vec![values[1]],
            writes_nzcv: false,
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
            crate::aarch64_physical_register_model(),
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
            for float_name in ["d0", "d31"] {
                for bits_name in ["x0", "x28"] {
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
