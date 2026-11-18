use selected_instructions::SaturatingCarrier;
use selected_instructions::SaturatingOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturatingForm {
    AddU64,
    SubtractUnsigned,
    DivideUnsigned,
    ClampSignedNarrow,
    ClampUnsignedNarrow,
    OverflowSelectI64,
    DivideSignedNarrow,
    DivideI64,
    RemainderUnsigned,
    RemainderSigned,
    MultiplySignedNarrow,
    MultiplyUnsignedNarrow,
    MultiplyI64,
    MultiplyU64,
}

impl SaturatingForm {
    pub const fn of(operation: SaturatingOperation, carrier: SaturatingCarrier) -> Self {
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
            (SaturatingOperation::Multiply, true, true) => Self::MultiplySignedNarrow,
            (SaturatingOperation::Multiply, false, true) => Self::MultiplyUnsignedNarrow,
            (SaturatingOperation::Multiply, true, false) => Self::MultiplyI64,
            (SaturatingOperation::Multiply, false, false) => Self::MultiplyU64,
        }
    }

    pub const fn byte_count(self) -> u16 {
        match self {
            Self::AddU64 => 19,
            Self::SubtractUnsigned => 13,
            Self::DivideUnsigned => 3,
            Self::ClampSignedNarrow => 40,
            Self::ClampUnsignedNarrow => 23,
            Self::OverflowSelectI64 => 25,
            Self::DivideSignedNarrow => 22,
            Self::DivideI64 => 26,
            Self::RemainderUnsigned => 9,
            Self::RemainderSigned => 17,
            Self::MultiplySignedNarrow => 41,
            Self::MultiplyUnsignedNarrow => 24,
            Self::MultiplyI64 => 29,
            Self::MultiplyU64 => 9,
        }
    }

    pub const fn operand_count(self) -> usize {
        match self {
            Self::AddU64 | Self::SubtractUnsigned => 3,
            Self::DivideUnsigned
            | Self::ClampSignedNarrow
            | Self::ClampUnsignedNarrow
            | Self::OverflowSelectI64
            | Self::DivideSignedNarrow
            | Self::DivideI64
            | Self::RemainderUnsigned
            | Self::RemainderSigned
            | Self::MultiplySignedNarrow
            | Self::MultiplyUnsignedNarrow
            | Self::MultiplyI64
            | Self::MultiplyU64 => 4,
        }
    }

    pub const fn is_division(self) -> bool {
        matches!(
            self,
            Self::DivideUnsigned
                | Self::DivideSignedNarrow
                | Self::DivideI64
                | Self::RemainderUnsigned
                | Self::RemainderSigned
        )
    }

    pub const fn is_fixed_rax_rdx(self) -> bool {
        self.is_division() || matches!(self, Self::MultiplyU64)
    }

    pub fn accepts_registers(self, registers: &[u8]) -> bool {
        if registers.len() != self.operand_count() {
            return false;
        }
        let (inputs, outputs) = registers.split_at(2);
        if self.is_fixed_rax_rdx() {
            return inputs[0] == 0 && outputs[0] == 0 && outputs[1] == 2 && inputs[1] != 2;
        }
        outputs.iter().all(|output| !inputs.contains(output))
            && (outputs.len() == 1 || outputs[0] != outputs[1])
    }

    pub fn operand_reads_and_writes(self) -> (Vec<u16>, Vec<u16>) {
        match self {
            Self::AddU64 | Self::SubtractUnsigned => (vec![0, 1], vec![2]),
            Self::DivideUnsigned | Self::DivideSignedNarrow | Self::DivideI64 => {
                (vec![0, 1, 3], vec![2])
            }
            Self::RemainderUnsigned | Self::RemainderSigned => (vec![0, 1], vec![2, 3]),
            Self::ClampSignedNarrow
            | Self::ClampUnsignedNarrow
            | Self::OverflowSelectI64
            | Self::MultiplySignedNarrow
            | Self::MultiplyUnsignedNarrow
            | Self::MultiplyI64
            | Self::MultiplyU64 => (vec![0, 1], vec![2, 3]),
        }
    }
}
