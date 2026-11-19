//! Core integer policy declarations retain their exact applied signatures.
//! Selecting one never makes its qualified operands eligible for builtin fallback.

use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_carrier::FixedInteger;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ScalarBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl ScalarBinaryOperation {
    pub const fn is_shift(self) -> bool {
        matches!(self, Self::ShiftLeft | Self::ShiftRight)
    }

    pub const fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Equal
                | Self::NotEqual
                | Self::Less
                | Self::LessOrEqual
                | Self::Greater
                | Self::GreaterOrEqual
        )
    }

    /// A shift's count keeps its own policy; the value selects the operation.
    pub fn selected_policy(
        self,
        left: ArithmeticDomain,
        right: ArithmeticDomain,
    ) -> Option<ArithmeticDomain> {
        use ArithmeticDomain::Exact;
        if self.is_shift() || right == Exact || right == left {
            Some(left)
        } else if left == Exact {
            Some(right)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct CoreIntegerOperand {
    pub carrier: FixedInteger,
    pub policy: ArithmeticDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreIntegerResult {
    Boolean,
    Integer(CoreIntegerOperand),
}

/// The catalog application is its operation and complete operand types.
/// Every positive fixed width uses this same declaration family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct CoreIntegerOperatorSignature {
    operation: ScalarBinaryOperation,
    operands: [CoreIntegerOperand; 2],
}

impl CoreIntegerOperatorSignature {
    pub fn select(
        operation: ScalarBinaryOperation,
        operands: [CoreIntegerOperand; 2],
    ) -> Option<Self> {
        use ArithmeticDomain::Exact;
        let [left, right] = operands;
        if left.policy == Exact && right.policy == Exact {
            return None;
        }
        if !operation.is_shift() && left.carrier != right.carrier {
            return None;
        }
        operation.selected_policy(left.policy, right.policy)?;
        Some(Self {
            operation,
            operands,
        })
    }

    pub const fn operation(self) -> ScalarBinaryOperation {
        self.operation
    }
    pub const fn operands(self) -> [CoreIntegerOperand; 2] {
        self.operands
    }

    pub fn is_valid(self) -> bool {
        Self::select(self.operation, self.operands) == Some(self)
    }

    pub fn result(self) -> Option<CoreIntegerResult> {
        if !self.is_valid() {
            return None;
        }
        if self.operation.is_comparison() {
            return Some(CoreIntegerResult::Boolean);
        }
        let [left, right] = self.operands;
        Some(CoreIntegerResult::Integer(CoreIntegerOperand {
            carrier: left.carrier,
            policy: self.operation.selected_policy(left.policy, right.policy)?,
        }))
    }
}
