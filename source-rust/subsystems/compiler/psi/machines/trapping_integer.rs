use semantic_vocabulary::ValueId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrappingIntegerOperation {
    Add { left: ValueId, right: ValueId },
    Subtract { left: ValueId, right: ValueId },
    Multiply { left: ValueId, right: ValueId },
    Divide { left: ValueId, right: ValueId },
    Remainder { left: ValueId, right: ValueId },
    ShiftLeft { value: ValueId, count: ValueId },
    ShiftRight { value: ValueId, count: ValueId },
    Convert { operand: ValueId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrappingIntegerPrimitive {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    Convert,
}

impl TrappingIntegerOperation {
    pub const fn primitive(self) -> TrappingIntegerPrimitive {
        match self {
            Self::Add { .. } => TrappingIntegerPrimitive::Add,
            Self::Subtract { .. } => TrappingIntegerPrimitive::Subtract,
            Self::Multiply { .. } => TrappingIntegerPrimitive::Multiply,
            Self::Divide { .. } => TrappingIntegerPrimitive::Divide,
            Self::Remainder { .. } => TrappingIntegerPrimitive::Remainder,
            Self::ShiftLeft { .. } => TrappingIntegerPrimitive::ShiftLeft,
            Self::ShiftRight { .. } => TrappingIntegerPrimitive::ShiftRight,
            Self::Convert { .. } => TrappingIntegerPrimitive::Convert,
        }
    }

    pub fn operands(self) -> Vec<ValueId> {
        match self {
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Remainder { left, right } => vec![left, right],
            Self::ShiftLeft { value, count } | Self::ShiftRight { value, count } => {
                vec![value, count]
            }
            Self::Convert { operand } => vec![operand],
        }
    }

    pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Remainder { left, right } => {
                *left = map(*left);
                *right = map(*right);
            }
            Self::ShiftLeft { value, count } | Self::ShiftRight { value, count } => {
                *value = map(*value);
                *count = map(*count);
            }
            Self::Convert { operand } => *operand = map(*operand),
        }
    }
}

#[macro_export]
macro_rules! trapping_arith_set_pattern {
    ($($fields:tt)*) => {
        $crate::TrappingIntegerOperation::Add { $($fields)* }
            | $crate::TrappingIntegerOperation::Divide { $($fields)* }
            | $crate::TrappingIntegerOperation::Multiply { $($fields)* }
            | $crate::TrappingIntegerOperation::Remainder { $($fields)* }
            | $crate::TrappingIntegerOperation::Subtract { $($fields)* }
    };
}
