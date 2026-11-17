/*
We specify the canonical AArch64 instruction-word realizations
(`SaturatingRealization`) for saturating integer addition, subtraction,
multiplication, and division across 8-, 16-, 32-, and 64-bit signed and unsigned
carriers so the machine-effect catalog, encoder, and structural decoder share a
single source of truth.
*/

use selected_instructions::{
    MachineSemanticKind, SaturatingCarrier, SaturatingOperation, SelectedInstructionKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaturatingRealization {
    AddU64,
    SubtractUnsigned,
    DivideUnsigned,
    ClampNarrow {
        operation: SaturatingOperation,
        carrier: SaturatingCarrier,
    },
    OverflowI64 {
        subtract: bool,
    },
    DivideI64,
    Remainder {
        signed: bool,
    },
    MultiplyI64,
    MultiplyU64,
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
            (SaturatingOperation::Multiply, SaturatingCarrier::I64) => Self::MultiplyI64,
            (SaturatingOperation::Multiply, SaturatingCarrier::U64) => Self::MultiplyU64,
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
            SelectedInstructionKind::SaturatingMultiply { carrier } => {
                Self::of(SaturatingOperation::Multiply, carrier)
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
            MachineSemanticKind::SaturatingMultiply(carrier) => {
                Self::of(SaturatingOperation::Multiply, carrier)
            }
            _ => return None,
        })
    }

    pub(crate) const fn operand_count(self) -> usize {
        match self {
            Self::AddU64
            | Self::SubtractUnsigned
            | Self::DivideUnsigned
            | Self::Remainder { .. } => 3,
            Self::ClampNarrow { .. }
            | Self::OverflowI64 { .. }
            | Self::DivideI64
            | Self::MultiplyI64
            | Self::MultiplyU64 => 4,
        }
    }

    pub(crate) const fn defines_nzcv(self) -> bool {
        !matches!(self, Self::DivideUnsigned | Self::Remainder { .. })
    }

    pub(crate) fn upper_condition(self) -> u32 {
        match self {
            Self::ClampNarrow {
                operation: SaturatingOperation::Multiply,
                carrier,
            } if !carrier.is_signed() => 0x8,
            _ => 0xc,
        }
    }

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
            Self::ClampNarrow { .. } => 4 + 12 * self.clamp_bounds().len() as u16,
            Self::OverflowI64 { .. } => 16,
            Self::DivideI64 => 24,
            Self::MultiplyI64 => 24,
            Self::MultiplyU64 => 16,
        }
    }
}
