//! Canonical register-only scalar FMA3 encodings.
//!
//! These helpers encode instruction mechanics only. Their presence does not
//! admit the FMA target feature or select an FMA provider for generic x86-64.

use omega_calling_conventions::MachineRegister;
use psi_diagnostics::Diagnostic;

/// Encode `VFMADD132SS destination, addend, multiplicand`.
///
/// The architectural result is the one-rounding binary32 operation
/// `destination * multiplicand + addend`. All three operands must be legacy
/// VEX-addressable XMM0..XMM15 registers.
pub fn encode_vfmadd132ss(
    destination: MachineRegister,
    addend: MachineRegister,
    multiplicand: MachineRegister,
) -> Result<[u8; 5], Diagnostic> {
    encode_vfmadd132_scalar(destination, addend, multiplicand, false, "VFMADD132SS")
}

/// Encode `VFMADD132SD destination, addend, multiplicand`.
///
/// The architectural result is the one-rounding binary64 operation
/// `destination * multiplicand + addend`. All three operands must be legacy
/// VEX-addressable XMM0..XMM15 registers.
pub fn encode_vfmadd132sd(
    destination: MachineRegister,
    addend: MachineRegister,
    multiplicand: MachineRegister,
) -> Result<[u8; 5], Diagnostic> {
    encode_vfmadd132_scalar(destination, addend, multiplicand, true, "VFMADD132SD")
}

fn encode_vfmadd132_scalar(
    destination: MachineRegister,
    addend: MachineRegister,
    multiplicand: MachineRegister,
    double_precision: bool,
    instruction: &str,
) -> Result<[u8; 5], Diagnostic> {
    let destination = xmm_index(destination, "destination", instruction)?;
    let addend = xmm_index(addend, "addend", instruction)?;
    let multiplicand = xmm_index(multiplicand, "multiplicand", instruction)?;

    // VEX.NDS.LIG.66.0F38.W{0,1} 99 /r. Register-only operands have no
    // index register, so inverted X stays set. R and B extend ModRM.reg and
    // ModRM.r/m; inverted vvvv names the addend.
    let vex_second = if destination < 8 { 0x80 } else { 0 }
        | 0x40
        | if multiplicand < 8 { 0x20 } else { 0 }
        | 0x02;
    let vex_third = if double_precision { 0x80 } else { 0 } | ((!addend & 0x0f) << 3) | 0x01;
    let mod_rm = 0xc0 | ((destination & 7) << 3) | (multiplicand & 7);
    Ok([0xc4, vex_second, vex_third, 0x99, mod_rm])
}

fn xmm_index(register: MachineRegister, role: &str, instruction: &str) -> Result<u8, Diagnostic> {
    match register {
        MachineRegister::X86Xmm(index @ 0..=15) => Ok(index),
        MachineRegister::X86Xmm(index) => Err(Diagnostic::error(format!(
            "{instruction} {role} XMM{index} is not addressable by canonical VEX encoding"
        ))),
        other => Err(Diagnostic::error(format!(
            "{instruction} {role} must be an x86-64 XMM register, got {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_canonical_low_register_scalar_fma3_forms() {
        assert_eq!(
            encode_vfmadd132ss(
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
                MachineRegister::X86Xmm(2),
            )
            .unwrap(),
            [0xc4, 0xe2, 0x71, 0x99, 0xc2]
        );
        assert_eq!(
            encode_vfmadd132sd(
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
                MachineRegister::X86Xmm(2),
            )
            .unwrap(),
            [0xc4, 0xe2, 0xf1, 0x99, 0xc2]
        );
    }

    #[test]
    fn encodes_canonical_extended_register_scalar_fma3_forms() {
        assert_eq!(
            encode_vfmadd132ss(
                MachineRegister::X86Xmm(9),
                MachineRegister::X86Xmm(10),
                MachineRegister::X86Xmm(15),
            )
            .unwrap(),
            [0xc4, 0x42, 0x29, 0x99, 0xcf]
        );
        assert_eq!(
            encode_vfmadd132sd(
                MachineRegister::X86Xmm(9),
                MachineRegister::X86Xmm(10),
                MachineRegister::X86Xmm(15),
            )
            .unwrap(),
            [0xc4, 0x42, 0xa9, 0x99, 0xcf]
        );
    }

    #[test]
    fn rejects_non_xmm_operands() {
        for invalid in [
            MachineRegister::X86Rax,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64V(0),
        ] {
            let destination = encode_vfmadd132ss(
                invalid,
                MachineRegister::X86Xmm(1),
                MachineRegister::X86Xmm(2),
            )
            .unwrap_err();
            assert!(destination.to_string().contains("destination must be"));

            let addend = encode_vfmadd132sd(
                MachineRegister::X86Xmm(0),
                invalid,
                MachineRegister::X86Xmm(2),
            )
            .unwrap_err();
            assert!(addend.to_string().contains("addend must be"));

            let multiplicand = encode_vfmadd132ss(
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
                invalid,
            )
            .unwrap_err();
            assert!(multiplicand.to_string().contains("multiplicand must be"));
        }
    }

    #[test]
    fn rejects_evex_only_xmm_registers() {
        for invalid in [16, 23, u8::MAX] {
            assert!(
                encode_vfmadd132ss(
                    MachineRegister::X86Xmm(invalid),
                    MachineRegister::X86Xmm(1),
                    MachineRegister::X86Xmm(2),
                )
                .unwrap_err()
                .to_string()
                .contains("not addressable by canonical VEX encoding")
            );
            assert!(
                encode_vfmadd132sd(
                    MachineRegister::X86Xmm(0),
                    MachineRegister::X86Xmm(invalid),
                    MachineRegister::X86Xmm(2),
                )
                .is_err()
            );
            assert!(
                encode_vfmadd132ss(
                    MachineRegister::X86Xmm(0),
                    MachineRegister::X86Xmm(1),
                    MachineRegister::X86Xmm(invalid),
                )
                .is_err()
            );
        }
    }
}
