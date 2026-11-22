#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerPrimitive {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Negate,
    ShiftLeft,
    ShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseNot,
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
}

/// The result and operand types on the operation supply each width and signedness.
/// Arithmetic traps when its result is outside the carrier. Division and remainder
/// also trap on zero and signed MIN by -1. Shifts check the original count
/// against the width of the left operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrappingIntegerPrimitive {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticPolicy {
    Exact,
    Wrapping,
    Saturating,
}

/// Each question applies to the operands and result of this operation. The
/// lowered-form checker must establish it before admitting the program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerObligation {
    ResultFitsCarrier,
    NonzeroDivisor,
    QuotientFitsCarrier,
    ShiftCountInRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerOperation {
    pub primitive: IntegerPrimitive,
    pub policy: ArithmeticPolicy,
    pub obligations: Vec<IntegerObligation>,
}

impl IntegerOperation {
    /// The formation questions for this primitive on an integer of the given sign.
    /// An unavailable arithmetic-policy spelling has no contract.
    pub fn required_obligations(
        primitive: IntegerPrimitive,
        policy: ArithmeticPolicy,
        signed: bool,
    ) -> Option<&'static [IntegerObligation]> {
        use ArithmeticPolicy::{Exact, Saturating, Wrapping};
        use IntegerObligation::*;
        use IntegerPrimitive::*;
        Some(match (policy, primitive) {
            (Exact, Negate) if !signed => return None,
            (Exact, Add | Subtract | Multiply | Negate) => &[ResultFitsCarrier],
            (Exact, Divide | Remainder) if signed => &[NonzeroDivisor, QuotientFitsCarrier],
            (_, Divide | Remainder) => &[NonzeroDivisor],
            (Exact, ShiftLeft) => &[ShiftCountInRange, ResultFitsCarrier],
            (Exact, ShiftRight) | (Saturating, ShiftLeft | ShiftRight) => &[ShiftCountInRange],
            (Wrapping | Saturating, Add | Subtract | Multiply)
            | (Wrapping, ShiftLeft | ShiftRight)
            | (Exact, BitwiseAnd | BitwiseOr | BitwiseXor | BitwiseNot | Equal | NotEqual | LessThan | LessOrEqual | GreaterThan | GreaterOrEqual) => &[],
            _ => return None,
        })
    }
}
