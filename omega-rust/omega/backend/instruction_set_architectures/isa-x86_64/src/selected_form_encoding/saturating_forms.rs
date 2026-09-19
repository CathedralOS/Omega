//! The x86-64 realization shape of each saturating operation and carrier.
//!
//! The scalar transport keeps narrow (8/16/32-bit) carriers sign- or
//! zero-normalized in 64-bit registers, so the 64-bit sum, difference, or
//! quotient of two normalized narrow operands is exact and only the final
//! clamp depends on the carrier. The 64-bit carriers need real overflow
//! detection: the u64 forms select on the borrow, the i64 add and subtract
//! select the saturated value on the overflow flag, and the i64 divide guards
//! the one faulting quotient before dividing. The encoder, the decoded-form
//! validator, the footprint, and the machine-effect catalog all read the
//! shape from here so their operand counts, sizes, and effects agree.

use selected_instructions::{SaturatingCarrier, SaturatingOperation, SelectedInstructionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaturatingForm {
    /// `mov; not; cmp; cmovb; sub; not` computes `min(left + right, u64::MAX)`
    /// as a three-operand carry select (19 bytes).
    AddU64,
    /// `cmp; mov; cmovb; sub` clamps every unsigned difference at zero (13
    /// bytes); a zero-normalized narrow difference borrows exactly when the
    /// u64 one does.
    SubtractUnsigned,
    /// `div` on the fixed RAX/RDX pair (3 bytes); an unsigned quotient never
    /// exceeds its carrier.
    DivideUnsigned,
    /// `mov; add/sub` into the early-clobber result, then a MOVABS/CMP/CMOVG
    /// upper clamp and a MOVABS/CMP/CMOVL lower clamp through the
    /// early-clobber bound scratch (40 bytes).
    ClampSignedNarrow,
    /// `mov; add` then only the MOVABS/CMP/CMOVG upper clamp (23 bytes): two
    /// zero-normalized operands below 2^32 sum below 2^33, so the signed
    /// compare against the carrier maximum is exact.
    ClampUnsignedNarrow,
    /// The scratch is derived from the left operand's sign before the
    /// arithmetic (`mov; not; sar 63; btc 63` yields i64::MAX for a
    /// non-negative left and i64::MIN otherwise), then `mov; add/sub; cmovo`
    /// replaces an overflowed result by that saturated value (25 bytes).
    OverflowSelectI64,
    /// `cqo; idiv` then the MOVABS/CMP/CMOVG upper clamp through RDX (22
    /// bytes); MIN / -1 is the only quotient outside a narrow signed carrier.
    DivideSignedNarrow,
    /// `cmp divisor, -1; sbb rdx, rdx; or rdx, rax; neg rdx; lea rdx,
    /// [rax + 1]; cmovo rax, rdx; cqo; idiv` (26 bytes): NEG overflows exactly
    /// when the divisor is -1 and the dividend is i64::MIN, and the dividend
    /// then becomes MIN + 1 so IDIV yields i64::MAX instead of faulting.
    DivideI64,
    /// `xor rdx, rdx; div divisor; mov rax, rdx` — the shared unsigned
    /// remainder shape (9 bytes). A zero-extended dividend cannot overflow
    /// the quotient, and a mathematical remainder already lies inside every
    /// carrier.
    RemainderUnsigned,
    /// `xor rdx, rdx; cmp divisor, -1; je over cqo/idiv; mov rax, rdx` —
    /// the shared signed remainder shape (17 bytes). x % -1 is zero for
    /// every dividend, so skipping IDIV leaves the pre-cleared zero and
    /// avoids the MIN / -1 quotient fault.
    RemainderSigned,
}

impl SaturatingForm {
    pub(crate) const fn of(operation: SaturatingOperation, carrier: SaturatingCarrier) -> Self {
        match (operation, carrier.is_signed(), carrier.is_narrow()) {
            (SaturatingOperation::Add, false, false) => Self::AddU64,
            (SaturatingOperation::Add, false, true) => Self::ClampUnsignedNarrow,
            (SaturatingOperation::Add | SaturatingOperation::Subtract, true, true) => {
                Self::ClampSignedNarrow
            }
            (SaturatingOperation::Add | SaturatingOperation::Subtract, true, false) => {
                Self::OverflowSelectI64
            }
            (SaturatingOperation::Subtract, false, _) => Self::SubtractUnsigned,
            (SaturatingOperation::Divide, false, _) => Self::DivideUnsigned,
            (SaturatingOperation::Divide, true, true) => Self::DivideSignedNarrow,
            (SaturatingOperation::Divide, true, false) => Self::DivideI64,
            (SaturatingOperation::Remainder, false, _) => Self::RemainderUnsigned,
            (SaturatingOperation::Remainder, true, _) => Self::RemainderSigned,
        }
    }

    pub(crate) const fn byte_count(self) -> u16 {
        match self {
            Self::AddU64 => 19,
            Self::SubtractUnsigned => 13,
            Self::DivideUnsigned => 3,
            // MOV/ADD (6) plus two MOVABS/CMP/CMOV clamps (17 each).
            Self::ClampSignedNarrow => 40,
            // MOV/ADD (6) plus one MOVABS/CMP/CMOV clamp (17).
            Self::ClampUnsignedNarrow => 23,
            // MOV/NOT/SAR/BTC (15) plus MOV/ADD/CMOVO (10).
            Self::OverflowSelectI64 => 25,
            // CQO/IDIV (5) plus one MOVABS/CMP/CMOV clamp (17).
            Self::DivideSignedNarrow => 22,
            // CMP/SBB/OR/NEG/LEA/CMOVO (21) plus CQO/IDIV (5).
            Self::DivideI64 => 26,
            // XOR (3) plus DIV (3) plus MOV (3).
            Self::RemainderUnsigned => 9,
            // XOR (3) plus CMP/JE (6) plus CQO/IDIV (5) plus MOV (3).
            Self::RemainderSigned => 17,
        }
    }

    /// Every division uses the fixed `[rax Use, divisor Use, rax Def, rdx]`
    /// shape of unsigned division, and both remainder forms share the same
    /// four-operand fixed row with RDX as an output; the clamped and
    /// overflow-select forms add an early-clobber scratch after the
    /// early-clobber result.
    pub(crate) const fn operand_count(self) -> usize {
        match self {
            Self::AddU64 | Self::SubtractUnsigned => 3,
            Self::DivideUnsigned
            | Self::ClampSignedNarrow
            | Self::ClampUnsignedNarrow
            | Self::OverflowSelectI64
            | Self::DivideSignedNarrow
            | Self::DivideI64
            | Self::RemainderUnsigned
            | Self::RemainderSigned => 4,
        }
    }

    /// Whether the form sits on the fixed RAX/RDX divide pair with the
    /// divisor kept out of RDX; the remainder forms share those pins.
    pub(crate) const fn is_division(self) -> bool {
        matches!(
            self,
            Self::DivideUnsigned
                | Self::DivideSignedNarrow
                | Self::DivideI64
                | Self::RemainderUnsigned
                | Self::RemainderSigned
        )
    }

    /// Whether resolved register codes satisfy the form's pins: every
    /// division sits on RAX with the divisor outside RDX (whose explicit input
    /// value CQO or the unsigned zero convention discards), and the outputs of
    /// every other form accumulate before the inputs are dead, so neither the
    /// early-clobber result nor the scratch may alias an input or each other.
    pub(crate) fn accepts_registers(self, registers: &[u8]) -> bool {
        if registers.len() != self.operand_count() {
            return false;
        }
        let (inputs, outputs) = registers.split_at(2);
        if self.is_division() {
            return inputs[0] == 0 && outputs[0] == 0 && outputs[1] == 2 && inputs[1] != 2;
        }
        outputs.iter().all(|output| !inputs.contains(output))
            && (outputs.len() == 1 || outputs[0] != outputs[1])
    }

    /// Operand positions read and written, as the machine-effect catalog and
    /// the encoded footprint declare them.
    pub(crate) fn operand_reads_and_writes(self) -> (Vec<u16>, Vec<u16>) {
        match self {
            Self::AddU64 | Self::SubtractUnsigned => (vec![0, 1], vec![2]),
            // Division reads the explicit RDX input that CQO or the zero
            // convention discards, and defines only RAX.
            Self::DivideUnsigned | Self::DivideSignedNarrow | Self::DivideI64 => {
                (vec![0, 1, 3], vec![2])
            }
            // The remainder forms define both outputs: RDX carries the
            // remainder until the final move into the RAX result home.
            Self::RemainderUnsigned | Self::RemainderSigned => (vec![0, 1], vec![2, 3]),
            Self::ClampSignedNarrow | Self::ClampUnsignedNarrow | Self::OverflowSelectI64 => {
                (vec![0, 1], vec![2, 3])
            }
        }
    }
}

/// The saturating operation and carrier of a selected kind, or `None` for
/// every other kind.
pub(crate) const fn saturating_operation(
    kind: SelectedInstructionKind,
) -> Option<(SaturatingOperation, SaturatingCarrier)> {
    match kind {
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            Some((SaturatingOperation::Add, carrier))
        }
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            Some((SaturatingOperation::Subtract, carrier))
        }
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
            Some((SaturatingOperation::Divide, carrier))
        }
        SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
            Some((SaturatingOperation::Remainder, carrier))
        }
        _ => None,
    }
}
