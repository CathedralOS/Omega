use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::SaturatingCarrier;
use selected_instructions::TrappingForm;
use selected_instructions::TrappingOperation;
use semantic_vocabulary::IntegerSign;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrappingRow {
    Binary,
    FixedPair,
    Shift,
    Convert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrappingArithmetic {
    Add,
    Subtract,
    Multiply,
}

impl TrappingArithmetic {
    pub const fn byte_count(self) -> u16 {
        match self {
            Self::Add | Self::Subtract => 3,
            Self::Multiply => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeCheck {
    SignExtension { bits: u16 },
    HighBits { shift: u8 },
}

impl RangeCheck {
    pub const fn of_narrow_carrier(carrier: SaturatingCarrier) -> Self {
        if carrier.is_signed() {
            Self::SignExtension {
                bits: carrier.bits(),
            }
        } else {
            Self::HighBits {
                shift: carrier.bits() as u8,
            }
        }
    }

    pub const fn of_conversion(source: IntegerSign, carrier: SaturatingCarrier) -> Option<Self> {
        match (source, carrier) {
            (IntegerSign::Signed, SaturatingCarrier::I64)
            | (IntegerSign::Unsigned, SaturatingCarrier::U64) => None,
            (IntegerSign::Signed, SaturatingCarrier::U64) => Some(Self::HighBits { shift: 63 }),
            (IntegerSign::Signed, _) if carrier.is_signed() => Some(Self::SignExtension {
                bits: carrier.bits(),
            }),
            _ if carrier.is_signed() => Some(Self::HighBits {
                shift: carrier.bits() as u8 - 1,
            }),
            _ => Some(Self::HighBits {
                shift: carrier.bits() as u8,
            }),
        }
    }

    pub const fn byte_count(self) -> u16 {
        match self {
            Self::SignExtension { bits: 32 } => 10,
            Self::SignExtension { .. } | Self::HighBits { .. } => 11,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividendGuard {
    Minimum8(i8),
    Minimum32(i32),
    DecrementOverflows,
}

impl DividendGuard {
    pub const fn of(carrier: SaturatingCarrier) -> Option<Self> {
        match carrier {
            SaturatingCarrier::I8 => Some(Self::Minimum8(i8::MIN)),
            SaturatingCarrier::I16 => Some(Self::Minimum32(i16::MIN as i32)),
            SaturatingCarrier::I32 => Some(Self::Minimum32(i32::MIN)),
            SaturatingCarrier::I64 => Some(Self::DecrementOverflows),
            SaturatingCarrier::U8
            | SaturatingCarrier::U16
            | SaturatingCarrier::U32
            | SaturatingCarrier::U64 => None,
        }
    }

    pub const fn byte_count(self) -> u16 {
        match self {
            Self::Minimum8(_) | Self::DecrementOverflows => 8,
            Self::Minimum32(_) => 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrappingShape {
    OverflowFlag(TrappingArithmetic),
    CarryFlag(TrappingArithmetic),
    NarrowRange(TrappingArithmetic, RangeCheck),
    MultiplyHighHalf,
    Division {
        guard: Option<DividendGuard>,
        remainder: bool,
    },
    ShiftLeftRoundTrip {
        signed: bool,
    },
    ShiftLeftRange {
        width: u8,
        check: RangeCheck,
    },
    ShiftRight {
        width: u8,
        signed: bool,
    },
    Convert(Option<RangeCheck>),
}

impl TrappingShape {
    pub const fn of(form: TrappingForm) -> Self {
        let carrier = form.carrier;
        let arithmetic = match form.operation {
            TrappingOperation::Add => TrappingArithmetic::Add,
            TrappingOperation::Subtract => TrappingArithmetic::Subtract,
            _ => TrappingArithmetic::Multiply,
        };
        match form.operation {
            TrappingOperation::Multiply if matches!(carrier, SaturatingCarrier::U64) => {
                Self::MultiplyHighHalf
            }
            TrappingOperation::Add | TrappingOperation::Subtract | TrappingOperation::Multiply => {
                match carrier {
                    SaturatingCarrier::I64 => Self::OverflowFlag(arithmetic),
                    SaturatingCarrier::U64 => Self::CarryFlag(arithmetic),
                    _ => Self::NarrowRange(arithmetic, RangeCheck::of_narrow_carrier(carrier)),
                }
            }
            TrappingOperation::Divide | TrappingOperation::Remainder => Self::Division {
                guard: DividendGuard::of(carrier),
                remainder: matches!(form.operation, TrappingOperation::Remainder),
            },
            TrappingOperation::ShiftLeft if carrier.is_narrow() => Self::ShiftLeftRange {
                width: carrier.bits() as u8,
                check: RangeCheck::of_narrow_carrier(carrier),
            },
            TrappingOperation::ShiftLeft => Self::ShiftLeftRoundTrip {
                signed: carrier.is_signed(),
            },
            TrappingOperation::ShiftRight => Self::ShiftRight {
                width: carrier.bits() as u8,
                signed: carrier.is_signed(),
            },
            TrappingOperation::Convert { source } => {
                Self::Convert(RangeCheck::of_conversion(source, carrier))
            }
        }
    }

    pub const fn row(self) -> TrappingRow {
        match self {
            Self::OverflowFlag(_) | Self::CarryFlag(_) | Self::NarrowRange(..) => {
                TrappingRow::Binary
            }
            Self::MultiplyHighHalf | Self::Division { .. } => TrappingRow::FixedPair,
            Self::ShiftLeftRoundTrip { .. }
            | Self::ShiftLeftRange { .. }
            | Self::ShiftRight { .. } => TrappingRow::Shift,
            Self::Convert(_) => TrappingRow::Convert,
        }
    }

    pub const fn byte_count(self) -> u16 {
        pub const TRAP_UNLESS: u16 = 4;
        pub const COUNT_CHECK: u16 = 4 + TRAP_UNLESS;
        pub const SHIFT: u16 = 6;
        match self {
            Self::OverflowFlag(arithmetic) | Self::CarryFlag(arithmetic) => {
                3 + arithmetic.byte_count() + TRAP_UNLESS
            }
            Self::NarrowRange(arithmetic, check) => {
                3 + arithmetic.byte_count() + check.byte_count()
            }
            Self::MultiplyHighHalf => 3 + TRAP_UNLESS,
            Self::Division { guard, remainder } => {
                let zero = 3 + TRAP_UNLESS;
                let divide = match guard {
                    None => 6,
                    Some(guard) => 4 + 2 + guard.byte_count() + 5,
                };
                zero + divide + if remainder { 3 } else { 0 }
            }
            Self::ShiftLeftRoundTrip { .. } => COUNT_CHECK + SHIFT + 9 + TRAP_UNLESS,
            Self::ShiftLeftRange { check, .. } => COUNT_CHECK + SHIFT + check.byte_count(),
            Self::ShiftRight { .. } => COUNT_CHECK + SHIFT,
            Self::Convert(None) => 3,
            Self::Convert(Some(check)) => check.byte_count() + 3,
        }
    }

    pub const fn operand_count(self) -> usize {
        match self.row() {
            TrappingRow::Binary | TrappingRow::FixedPair | TrappingRow::Shift => 4,
            TrappingRow::Convert => 3,
        }
    }

    pub fn accepts_registers(self, registers: &[u8]) -> bool {
        if registers.len() != self.operand_count() {
            return false;
        }
        let (inputs, outputs) = registers.split_at(registers.len() - 2);
        match self.row() {
            TrappingRow::FixedPair => {
                inputs[0] == 0 && outputs[0] == 0 && outputs[1] == 2 && inputs[1] != 2
            }
            TrappingRow::Shift if inputs[1] != 1 => false,
            TrappingRow::Binary | TrappingRow::Shift | TrappingRow::Convert => {
                outputs.iter().all(|output| !inputs.contains(output)) && outputs[0] != outputs[1]
            }
        }
    }

    pub fn operand_reads_and_writes(self) -> (Vec<u16>, Vec<u16>) {
        match self.row() {
            TrappingRow::Binary | TrappingRow::FixedPair | TrappingRow::Shift => {
                (vec![0, 1], vec![2, 3])
            }
            TrappingRow::Convert => (vec![0], vec![1, 2]),
        }
    }

    pub fn encoded_effects(self) -> MachineEncodedEffects {
        let (reads, writes) = self.operand_reads_and_writes();
        let mut encoded = MachineEncodedEffects::fallthrough_v1(reads, writes);
        encoded.implicit_unit_clobbers = crate::x86_64_physical_register_model()
            .view_named("condition_flags")
            .expect("canonical x86-64 model declares condition flags")
            .units
            .clone();
        encoded.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        encoded.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
        encoded
    }
}
