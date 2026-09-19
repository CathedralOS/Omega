//! How each saturating operation and carrier is realized on AArch64.
//!
//! The scalar transport keeps narrow (8/16/32-bit) carriers sign- or
//! zero-normalized in 64-bit registers, so their exact 64-bit sum,
//! difference, or quotient fits and only the final clamp depends on the
//! carrier. The 64-bit carriers cannot rely on that headroom: u64 add and
//! subtract select on the carry flag, i64 add and subtract select on the
//! overflow flag, and i64 divide must recognize the one quotient (MIN / -1)
//! that `sdiv` wraps instead of clamping. Unsigned division never overflows.
//! Every realization here is one fixed word sequence, so the machine-effect
//! catalog, the encoder, the decoder check, and the footprint all consult the
//! same table rather than repeating the carrier classification.
use selected_instructions::{
    MachineSemanticKind, SaturatingCarrier, SaturatingOperation, SelectedInstructionKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaturatingRealization {
    /// `adds; csinv cs`: the u64 sum saturates on the carry flag.
    AddU64,
    /// `subs; csel cs`: every unsigned carrier clamps at zero on borrow,
    /// because a zero-normalized narrow difference borrows exactly when the
    /// u64 difference does.
    SubtractUnsigned,
    /// `udiv`: unsigned division never overflows; the zero divisor remains
    /// the proof obligation carried by the instruction.
    DivideUnsigned,
    /// Exact 64-bit arithmetic on normalized narrow operands, then a
    /// `mov/cmp/csel` clamp against each carrier bound held in the scratch:
    /// both bounds for signed add and subtract, the maximum alone for
    /// unsigned add (the sum cannot go below zero) and for signed divide
    /// (whose only out-of-range quotient is MIN / -1, which exceeds MAX).
    ClampNarrow {
        operation: SaturatingOperation,
        carrier: SaturatingCarrier,
    },
    /// `asr scratch, left, #63; eor scratch, scratch, #i64::MAX` derives the
    /// saturated value from the left operand's sign (MAX when non-negative,
    /// MIN when negative, which is the overflow direction for both add and
    /// subtract), then `adds`/`subs` and `csel vs` select it on overflow.
    OverflowI64 { subtract: bool },
    /// `sdiv` wraps MIN / -1 to MIN. `cmp result, MIN` then
    /// `ccmn divisor, #1, #0, eq` leaves Z set exactly when the result is MIN
    /// and the divisor is -1, and `csel eq` substitutes MAX.
    DivideI64,
    /// `sdiv`/`udiv` into the result followed by `msub` recovering
    /// `dividend - quotient * divisor`. A mathematical remainder already lies
    /// inside every carrier, so no clamp applies; the wrapped MIN / -1
    /// quotient still leaves the correct zero remainder through `msub`.
    Remainder { signed: bool },
}

impl SaturatingRealization {
    pub(crate) fn of(operation: SaturatingOperation, carrier: SaturatingCarrier) -> Self {
        match (operation, carrier) {
            (SaturatingOperation::Add, SaturatingCarrier::U64) => Self::AddU64,
            (SaturatingOperation::Subtract, carrier) if !carrier.is_signed() => {
                Self::SubtractUnsigned
            }
            (SaturatingOperation::Divide, carrier) if !carrier.is_signed() => Self::DivideUnsigned,
            (SaturatingOperation::Add, SaturatingCarrier::I64) => {
                Self::OverflowI64 { subtract: false }
            }
            (SaturatingOperation::Subtract, SaturatingCarrier::I64) => {
                Self::OverflowI64 { subtract: true }
            }
            (SaturatingOperation::Divide, SaturatingCarrier::I64) => Self::DivideI64,
            (SaturatingOperation::Remainder, carrier) => Self::Remainder {
                signed: carrier.is_signed(),
            },
            (operation, carrier) => Self::ClampNarrow { operation, carrier },
        }
    }

    pub(crate) fn of_kind(kind: SelectedInstructionKind) -> Option<Self> {
        Some(match kind {
            SelectedInstructionKind::SaturatingAdd { carrier } => {
                Self::of(SaturatingOperation::Add, carrier)
            }
            SelectedInstructionKind::SaturatingSubtract { carrier } => {
                Self::of(SaturatingOperation::Subtract, carrier)
            }
            SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
                Self::of(SaturatingOperation::Divide, carrier)
            }
            SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
                Self::of(SaturatingOperation::Remainder, carrier)
            }
            _ => return None,
        })
    }

    pub(crate) fn of_semantic(semantic: MachineSemanticKind) -> Option<Self> {
        Some(match semantic {
            MachineSemanticKind::SaturatingAdd(carrier) => {
                Self::of(SaturatingOperation::Add, carrier)
            }
            MachineSemanticKind::SaturatingSubtract(carrier) => {
                Self::of(SaturatingOperation::Subtract, carrier)
            }
            MachineSemanticKind::SaturatingDivide(carrier) => {
                Self::of(SaturatingOperation::Divide, carrier)
            }
            MachineSemanticKind::SaturatingRemainder(carrier) => {
                Self::of(SaturatingOperation::Remainder, carrier)
            }
            _ => return None,
        })
    }

    /// Three operands (left, right, result) for the flag-select, plain
    /// divide, and remainder forms; four when a bound scratch is clamped
    /// through.
    pub(crate) const fn operand_count(self) -> usize {
        match self {
            Self::AddU64
            | Self::SubtractUnsigned
            | Self::DivideUnsigned
            | Self::Remainder { .. } => 3,
            Self::ClampNarrow { .. } | Self::OverflowI64 { .. } | Self::DivideI64 => 4,
        }
    }

    /// Only the plain `udiv` and the divide/`msub` remainder pair leave NZCV
    /// alone.
    pub(crate) const fn defines_nzcv(self) -> bool {
        !matches!(self, Self::DivideUnsigned | Self::Remainder { .. })
    }

    /// The carrier bounds the narrow clamp materializes, in the order they
    /// are compared: the maximum first, then the minimum when both apply.
    pub(crate) fn clamp_bounds(self) -> Vec<u64> {
        match self {
            Self::ClampNarrow { operation, carrier } => {
                let mut bounds = vec![carrier.maximum_bits()];
                if carrier.is_signed() && operation != SaturatingOperation::Divide {
                    bounds.push(carrier.minimum_bits());
                }
                bounds
            }
            _ => Vec::new(),
        }
    }

    pub(crate) fn byte_size(self) -> u16 {
        match self {
            Self::AddU64 | Self::SubtractUnsigned | Self::Remainder { .. } => 8,
            Self::DivideUnsigned => 4,
            // The arithmetic word plus three words per clamped bound.
            Self::ClampNarrow { .. } => 4 + 12 * self.clamp_bounds().len() as u16,
            Self::OverflowI64 { .. } => 16,
            Self::DivideI64 => 24,
        }
    }
}
