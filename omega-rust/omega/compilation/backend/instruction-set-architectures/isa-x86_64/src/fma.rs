//! Canonical register-only scalar FMA3 encodings.
//!
//! These helpers encode instruction mechanics only. Their presence does not
//! admit the FMA target feature or select an FMA provider for generic x86-64.

use calling_conventions::MachineRegister;
use diagnostics::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodedScalarFmaFormat {
    Binary32,
    Binary64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedVfmadd132Scalar {
    pub format: DecodedScalarFmaFormat,
    pub destination: MachineRegister,
    pub addend: MachineRegister,
    pub multiplicand: MachineRegister,
}



/// Decode one exact canonical register-only scalar `VFMADD132S{S,D}`.
///
/// This is a structural replay of the VEX fields, not a call back into the
/// producer encoder. Noncanonical prefixes, memory operands, alternate FMA
/// operand permutations, and trailing bytes reject.
pub fn decode_vfmadd132_scalar(bytes: &[u8]) -> Result<DecodedVfmadd132Scalar, Diagnostic> {
    let [vex, vex_second, vex_third, opcode, mod_rm] = bytes else {
        return Err(Diagnostic::error(format!(
            "canonical scalar VFMADD132 encoding must contain exactly 5 bytes, got {}",
            bytes.len()
        )));
    };
    if *vex != 0xc4
        || (*vex_second & 0x5f) != 0x42
        || (*vex_third & 0x07) != 0x01
        || *opcode != 0x99
        || (*mod_rm & 0xc0) != 0xc0
    {
        return Err(Diagnostic::error(
            "bytes are not a canonical register-only scalar VFMADD132SS/SD encoding",
        ));
    }

    let format = if (*vex_third & 0x80) == 0 {
        DecodedScalarFmaFormat::Binary32
    } else {
        DecodedScalarFmaFormat::Binary64
    };
    let destination = ((*mod_rm >> 3) & 7) | (u8::from((*vex_second & 0x80) == 0) << 3);
    let multiplicand = (*mod_rm & 7) | (u8::from((*vex_second & 0x20) == 0) << 3);
    let addend = !(*vex_third >> 3) & 0x0f;
    Ok(DecodedVfmadd132Scalar {
        format,
        destination: MachineRegister::X86Xmm(destination),
        addend: MachineRegister::X86Xmm(addend),
        multiplicand: MachineRegister::X86Xmm(multiplicand),
    })
}



#[cfg(test)]
mod tests {
    use super::{
        DecodedScalarFmaFormat, DecodedVfmadd132Scalar, MachineRegister, decode_vfmadd132_scalar,
    };





    #[test]
    fn independently_decodes_both_formats_and_extended_registers() {
        for (bytes, expected) in [
            (
                [0xc4, 0xe2, 0x71, 0x99, 0xc2],
                DecodedVfmadd132Scalar {
                    format: DecodedScalarFmaFormat::Binary32,
                    destination: MachineRegister::X86Xmm(0),
                    addend: MachineRegister::X86Xmm(1),
                    multiplicand: MachineRegister::X86Xmm(2),
                },
            ),
            (
                [0xc4, 0x42, 0xa9, 0x99, 0xcf],
                DecodedVfmadd132Scalar {
                    format: DecodedScalarFmaFormat::Binary64,
                    destination: MachineRegister::X86Xmm(9),
                    addend: MachineRegister::X86Xmm(10),
                    multiplicand: MachineRegister::X86Xmm(15),
                },
            ),
        ] {
            assert_eq!(decode_vfmadd132_scalar(&bytes).unwrap(), expected);
        }
    }

    #[test]
    fn decoder_rejects_noncanonical_or_non_132_scalar_forms() {
        let canonical = [0xc4, 0xe2, 0x71, 0x99, 0xc2];
        for (index, replacement) in [(0, 0xc5), (1, 0xa2), (2, 0x75), (3, 0x98), (4, 0x02)] {
            let mut mutated = canonical;
            mutated[index] = replacement;
            assert!(decode_vfmadd132_scalar(&mutated).is_err());
        }
        assert!(decode_vfmadd132_scalar(&canonical[..4]).is_err());
        let mut trailing = canonical.to_vec();
        trailing.push(0x90);
        assert!(decode_vfmadd132_scalar(&trailing).is_err());
    }
}
