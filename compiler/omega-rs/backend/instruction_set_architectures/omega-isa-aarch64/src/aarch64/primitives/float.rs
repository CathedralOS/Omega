//! AArch64 scalar floating-point primitive encoders.
//!
//! These cover the scalar IEEE-754 operations the runtime-storage float lane
//! needs: binary arithmetic (`FADD`/`FSUB`/`FMUL`/`FDIV`), comparison (`FCMP`),
//! moves between the general-purpose and FP register banks (`FMOV`), precision
//! conversion (`FCVT`), and int<->float conversion (`SCVTF`/`FCVTZS`).
//!
//! Precision is selected by the operand byte width: a 4-byte operand uses the
//! single-precision `S<n>` registers (FP `ftype = 0b00`), an 8-byte operand uses
//! the double-precision `D<n>` registers (`ftype = 0b01`). This mirrors how the
//! x86_64 backend selects `addss`/`ucomiss` (single) vs `addsd`/`ucomisd`
//! (double) from the same `byte_size`.
//!
//! All bit patterns were cross-checked against an assembler (clang
//! `--target=aarch64-*`); see the unit tests in this module for the exact bytes.

use psi_diagnostics::Diagnostic;

use super::instruction::encode_instruction;

/// The `ftype` field (instruction bits [23:22]) that selects scalar precision.
///
/// `0b00` = single (32-bit), `0b01` = double (64-bit). Returns the field already
/// shifted into position so it can be OR-ed straight into a base word.
fn float_type_field(byte_size: usize) -> Result<u32, Diagnostic> {
    match byte_size {
        4 => Ok(0b00 << 22),
        8 => Ok(0b01 << 22),
        _ => Err(Diagnostic::error(format!(
            "AArch64 encoder only supports 4-byte (single) or 8-byte (double) scalar floats, got `{byte_size}`"
        ))),
    }
}

/// Floating-point data-processing (2 source): `op Vd, Vn, Vm`.
///
/// Encoding: `0001 1110 ftype 1 Rm opcode 10 Rn Rd`. The base word for single
/// precision is `0x1E20_0800`; `ftype` adds the double bit, `opcode` (bits
/// [15:12]) selects the operation.
fn encode_float_data_processing_2(
    opcode: u32,
    byte_size: usize,
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let base = 0x1E20_0800 | float_type_field(byte_size)?;
    Ok(encode_instruction(
        base | (opcode << 12)
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    ))
}

/// `FADD Vd, Vn, Vm` — scalar floating-point add. `byte_size` 4 → `Sd`, 8 → `Dd`.
pub(in crate::aarch64) fn encode_float_add(
    byte_size: usize,
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    encode_float_data_processing_2(
        0b0010,
        byte_size,
        destination_register,
        left_register,
        right_register,
    )
}

/// `FSUB Vd, Vn, Vm` — scalar floating-point subtract.
pub(in crate::aarch64) fn encode_float_subtract(
    byte_size: usize,
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    encode_float_data_processing_2(
        0b0011,
        byte_size,
        destination_register,
        left_register,
        right_register,
    )
}

/// `FMUL Vd, Vn, Vm` — scalar floating-point multiply.
pub(in crate::aarch64) fn encode_float_multiply(
    byte_size: usize,
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    encode_float_data_processing_2(
        0b0000,
        byte_size,
        destination_register,
        left_register,
        right_register,
    )
}

/// `FMADD Vd, Vn, Vm, Va` — scalar fused multiply-add. The multiplication
/// and addition round only once, as required by the named FMA contract.
pub(in crate::aarch64) fn encode_float_fused_multiply_add(
    byte_size: usize,
    destination_register: u8,
    left_register: u8,
    right_register: u8,
    addend_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let base = 0x1F00_0000 | float_type_field(byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(right_register) << 16)
            | (u32::from(addend_register) << 10)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    ))
}

/// `FDIV Vd, Vn, Vm` — scalar floating-point divide.
pub(in crate::aarch64) fn encode_float_divide(
    byte_size: usize,
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    encode_float_data_processing_2(
        0b0001,
        byte_size,
        destination_register,
        left_register,
        right_register,
    )
}

/// `FCMP Vn, Vm` — scalar floating-point compare, sets NZCV.
///
/// Encoding: `0001 1110 ftype 1 Rm 00 1000 Rn 0 0000`. Single base
/// `0x1E20_2000`. The result NZCV uses the C/Z/N flags exactly like an unsigned
/// integer compare for the ordered relations; the V/unordered (NaN) case is a
/// documented first-cut limitation (matches x86 `ucomis*`).
/// `FCSEL Vd, Vn, Vm, <cond>` -- select Vn when `<cond>` holds after an
/// `FCMP`, else Vm. Condition GT (0b1100) and MI (0b0100) are both FALSE on an
/// unordered compare, which is exactly what realizes the SSE
/// `maxsd`/`minsd` semantics (`a > b ? a : b`; NaN or equal returns b) the
/// interpreter pins.
pub(in crate::aarch64) fn encode_float_conditional_select(
    byte_size: usize,
    destination_register: u8,
    true_register: u8,
    false_register: u8,
    condition: u32,
) -> Result<[u8; 4], Diagnostic> {
    let base = 0x1E20_0C00 | float_type_field(byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(false_register) << 16)
            | (condition << 12)
            | (u32::from(true_register) << 5)
            | u32::from(destination_register),
    ))
}

/// `FSQRT Vd, Vn` -- scalar square root (IEEE-correct rounding, matching
/// `sqrtsd` exactly).
pub(in crate::aarch64) fn encode_float_sqrt(
    byte_size: usize,
    destination_register: u8,
    source_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let base = 0x1E21_C000 | float_type_field(byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(source_register) << 5) | u32::from(destination_register),
    ))
}

pub(in crate::aarch64) fn encode_float_compare(
    byte_size: usize,
    left_register: u8,
    right_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let base = 0x1E20_2000 | float_type_field(byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(right_register) << 16) | (u32::from(left_register) << 5),
    ))
}

/// `FMOV Sd, Wn` / `FMOV Dd, Xn` — move raw bits from a general-purpose register
/// into an FP register (no numeric conversion). `byte_size` 4 → `Sd, Wn`,
/// 8 → `Dd, Xn`.
///
/// Encoding `sf 0 011110 ftype 1 00111 000000 Rn Rd`. Single: `0x1E27_0000`,
/// double: `0x9E67_0000`.
pub(in crate::aarch64) fn encode_float_move_from_gpr(
    byte_size: usize,
    destination_fp_register: u8,
    source_gpr: u8,
) -> Result<[u8; 4], Diagnostic> {
    let base = match byte_size {
        4 => 0x1E27_0000,
        8 => 0x9E67_0000,
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 encoder only supports 4/8-byte FMOV from GPR, got `{byte_size}`"
            )));
        }
    };
    Ok(encode_instruction(
        base | (u32::from(source_gpr) << 5) | u32::from(destination_fp_register),
    ))
}

/// `FMOV Wd, Sn` / `FMOV Xd, Dn` — move raw bits from an FP register into a
/// general-purpose register (no numeric conversion).
///
/// Encoding `sf 0 011110 ftype 1 00110 000000 Rn Rd`. Single: `0x1E26_0000`,
/// double: `0x9E66_0000`.
pub(in crate::aarch64) fn encode_float_move_to_gpr(
    byte_size: usize,
    destination_gpr: u8,
    source_fp_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let base = match byte_size {
        4 => 0x1E26_0000,
        8 => 0x9E66_0000,
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 encoder only supports 4/8-byte FMOV to GPR, got `{byte_size}`"
            )));
        }
    };
    Ok(encode_instruction(
        base | (u32::from(source_fp_register) << 5) | u32::from(destination_gpr),
    ))
}

/// `FCVT Dd, Sn` — widen single -> double. Encoding `0x1E22_C000`.
///
/// Ready for the numeric `as` cast lowering; not yet wired (see
/// `runtime_storage::encode_runtime_storage_convert`).
#[allow(dead_code)]
pub(in crate::aarch64) fn encode_float_convert_single_to_double(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1E22_C000 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

/// `FCVT Sd, Dn` — narrow double -> single. Encoding `0x1E62_4000`.
///
/// Ready for the numeric `as` cast lowering; not yet wired.
#[allow(dead_code)]
pub(in crate::aarch64) fn encode_float_convert_double_to_single(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1E62_4000 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

/// `SCVTF Vd, Rn` — convert a signed integer in a GPR to a float in an FP
/// register. `int_byte_size` selects the GPR width (4 → `Wn`, 8 → `Xn`) and
/// `float_byte_size` selects the destination precision (4 → `Sd`, 8 → `Dd`).
///
/// Encoding `sf 0 011110 ftype 1 00 010 000000 Rn Rd`. The `sf` bit (31) comes
/// from the integer width, `ftype` from the float width.
///
/// Ready for the numeric `as` cast lowering; not yet wired.
#[allow(dead_code)]
pub(in crate::aarch64) fn encode_signed_int_to_float(
    int_byte_size: usize,
    float_byte_size: usize,
    destination_fp_register: u8,
    source_gpr: u8,
) -> Result<[u8; 4], Diagnostic> {
    let sf = int_width_sf_bit(int_byte_size)?;
    let base = 0x1E22_0000 | sf | float_type_field(float_byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(source_gpr) << 5) | u32::from(destination_fp_register),
    ))
}

/// `UCVTF Vd, Rn` — convert an unsigned integer in a GPR to a float in an FP
/// register. Width selection mirrors [`encode_signed_int_to_float`].
pub(in crate::aarch64) fn encode_unsigned_int_to_float(
    int_byte_size: usize,
    float_byte_size: usize,
    destination_fp_register: u8,
    source_gpr: u8,
) -> Result<[u8; 4], Diagnostic> {
    let sf = int_width_sf_bit(int_byte_size)?;
    let base = 0x1E23_0000 | sf | float_type_field(float_byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(source_gpr) << 5) | u32::from(destination_fp_register),
    ))
}

/// `FCVTZS Rd, Vn` — convert a float to a signed integer, rounding toward zero.
/// `float_byte_size` selects the source precision and `int_byte_size` the GPR
/// width (4 → `Wd`, 8 → `Xd`).
///
/// Encoding `sf 0 011110 ftype 1 11 000 000000 Rn Rd`.
///
/// Ready for the numeric `as` cast lowering; not yet wired.
#[allow(dead_code)]
pub(in crate::aarch64) fn encode_float_to_signed_int(
    float_byte_size: usize,
    int_byte_size: usize,
    destination_gpr: u8,
    source_fp_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let sf = int_width_sf_bit(int_byte_size)?;
    let base = 0x1E38_0000 | sf | float_type_field(float_byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(source_fp_register) << 5) | u32::from(destination_gpr),
    ))
}

/// `FCVTZU Rd, Vn` — convert a float to an unsigned integer, rounding toward
/// zero. Width selection mirrors [`encode_float_to_signed_int`].
pub(in crate::aarch64) fn encode_float_to_unsigned_int(
    float_byte_size: usize,
    int_byte_size: usize,
    destination_gpr: u8,
    source_fp_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let sf = int_width_sf_bit(int_byte_size)?;
    let base = 0x1E39_0000 | sf | float_type_field(float_byte_size)?;
    Ok(encode_instruction(
        base | (u32::from(source_fp_register) << 5) | u32::from(destination_gpr),
    ))
}

/// The `sf` bit (instruction bit 31), already shifted, that selects the GPR
/// width for int<->float conversions: 4-byte `W` → 0, 8-byte `X` → bit 31.
#[allow(dead_code)]
fn int_width_sf_bit(int_byte_size: usize) -> Result<u32, Diagnostic> {
    match int_byte_size {
        1 | 2 | 4 => Ok(0),
        8 => Ok(1 << 31),
        _ => Err(Diagnostic::error(format!(
            "AArch64 encoder only supports 4/8-byte integers in float conversions, got `{int_byte_size}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(bytes: [u8; 4]) -> u32 {
        u32::from_le_bytes(bytes)
    }

    // Reference bytes confirmed via `clang --target=aarch64-linux-gnu` +
    // `llvm-objdump -d`. AArch64 is little-endian, so e.g. `fadd s0,s1,s2`
    // disassembles to the word 0x1e222820.

    #[test]
    fn single_precision_binary_ops_match_arm_arm() {
        // FADD s0, s1, s2
        assert_eq!(word(encode_float_add(4, 0, 1, 2).unwrap()), 0x1e22_2820);
        // FSUB s0, s1, s2
        assert_eq!(
            word(encode_float_subtract(4, 0, 1, 2).unwrap()),
            0x1e22_3820
        );
        // FMUL s0, s1, s2
        assert_eq!(
            word(encode_float_multiply(4, 0, 1, 2).unwrap()),
            0x1e22_0820
        );
        // FDIV s0, s1, s2
        assert_eq!(word(encode_float_divide(4, 0, 1, 2).unwrap()), 0x1e22_1820);
    }

    #[test]
    fn double_precision_binary_ops_match_arm_arm() {
        // FADD d0, d1, d2
        assert_eq!(word(encode_float_add(8, 0, 1, 2).unwrap()), 0x1e62_2820);
        // FDIV d0, d1, d2
        assert_eq!(word(encode_float_divide(8, 0, 1, 2).unwrap()), 0x1e62_1820);
    }

    #[test]
    fn fused_multiply_add_matches_arm_arm() {
        // FMADD s0, s1, s2, s3 / FMADD d0, d1, d2, d3.
        assert_eq!(
            word(encode_float_fused_multiply_add(4, 0, 1, 2, 3).unwrap()),
            0x1f02_0c20
        );
        assert_eq!(
            word(encode_float_fused_multiply_add(8, 0, 1, 2, 3).unwrap()),
            0x1f42_0c20
        );
    }

    #[test]
    fn float_compare_matches_arm_arm() {
        // FCMP s1, s2
        assert_eq!(word(encode_float_compare(4, 1, 2).unwrap()), 0x1e22_2020);
        // FCMP d1, d2
        assert_eq!(word(encode_float_compare(8, 1, 2).unwrap()), 0x1e62_2020);
    }

    #[test]
    fn fmov_between_banks_matches_arm_arm() {
        // FMOV s0, w0  /  FMOV w0, s0
        assert_eq!(
            word(encode_float_move_from_gpr(4, 0, 0).unwrap()),
            0x1e27_0000
        );
        assert_eq!(
            word(encode_float_move_to_gpr(4, 0, 0).unwrap()),
            0x1e26_0000
        );
        // FMOV d0, x0  /  FMOV x0, d0
        assert_eq!(
            word(encode_float_move_from_gpr(8, 0, 0).unwrap()),
            0x9e67_0000
        );
        assert_eq!(
            word(encode_float_move_to_gpr(8, 0, 0).unwrap()),
            0x9e66_0000
        );
    }

    #[test]
    fn fcvt_precision_changes_match_arm_arm() {
        // FCVT d0, s0  (single -> double)
        assert_eq!(
            word(encode_float_convert_single_to_double(0, 0)),
            0x1e22_c000
        );
        // FCVT s0, d0  (double -> single)
        assert_eq!(
            word(encode_float_convert_double_to_single(0, 0)),
            0x1e62_4000
        );
    }

    #[test]
    fn unsigned_integer_conversions_match_arm_arm() {
        assert_eq!(
            word(encode_unsigned_int_to_float(4, 4, 0, 0).unwrap()),
            0x1e23_0000,
            "UCVTF s0, w0",
        );
        assert_eq!(
            word(encode_unsigned_int_to_float(8, 8, 0, 0).unwrap()),
            0x9e63_0000,
            "UCVTF d0, x0",
        );
    }

    #[test]
    fn int_to_float_matches_arm_arm() {
        // SCVTF s0, w0  (32-bit int -> single)
        assert_eq!(
            word(encode_signed_int_to_float(4, 4, 0, 0).unwrap()),
            0x1e22_0000
        );
        // SCVTF d0, x0  (64-bit int -> double)
        assert_eq!(
            word(encode_signed_int_to_float(8, 8, 0, 0).unwrap()),
            0x9e62_0000
        );
    }

    #[test]
    fn float_to_int_matches_arm_arm() {
        // FCVTZS w0, s0  (single -> 32-bit int)
        assert_eq!(
            word(encode_float_to_signed_int(4, 4, 0, 0).unwrap()),
            0x1e38_0000
        );
        // FCVTZS x0, d0  (double -> 64-bit int)
        assert_eq!(
            word(encode_float_to_signed_int(8, 8, 0, 0).unwrap()),
            0x9e78_0000
        );
        assert_eq!(
            word(encode_float_to_unsigned_int(4, 4, 0, 0).unwrap()),
            0x1e39_0000
        );
        assert_eq!(
            word(encode_float_to_unsigned_int(8, 8, 0, 0).unwrap()),
            0x9e79_0000
        );
    }

    #[test]
    fn register_fields_are_placed_correctly() {
        // FADD s7, s10, s21 -> Rd=7, Rn=10, Rm=21
        let w = word(encode_float_add(4, 7, 10, 21).unwrap());
        assert_eq!(w & 0x1f, 7, "Rd");
        assert_eq!((w >> 5) & 0x1f, 10, "Rn");
        assert_eq!((w >> 16) & 0x1f, 21, "Rm");
    }

    #[test]
    fn unsupported_width_is_rejected() {
        assert!(encode_float_add(2, 0, 1, 2).is_err());
        assert!(encode_float_compare(16, 0, 1).is_err());
        assert!(encode_float_move_from_gpr(2, 0, 0).is_err());
    }
}
