/*
We represent fixed-width integer arithmetic and shift operations as an explicit
(overflow policy, operator, obligation) tuple rather than a cross-product of
nineteen operation variants.

Terminal Psi names every admitted policy-and-operator pair as a separate
operation (`WrappingIntegerAdd`, `ExactIntegerDivide`, ...). Mirroring that
cross product across `AbstractOperation` and `TargetIntegerExpression` forced
consumers to repeat fifteen to nineteen arms that differed only in which pair
they named. Keeping the policy and operator as data lets consumers branch only
where the policy or operator changes behavior.

The admitted pairs do not share one uniform obligation shape: add, subtract, and
multiply carry a verifier-discharged range obligation only under `Exact`; divide
and remainder carry a nonzero-divisor obligation under every policy; shifts
exist only as `Wrapping` and `Exact`, and only `Exact` carries one. We keep the
fields of `IntegerArithmetic` private so `IntegerArithmetic::new` refuses any
pair whose obligation is missing or extraneous, and we embed the shift
obligation inside `IntegerShiftPolicy::Exact`.
*/

use semantic_vocabulary::{IntegerType, IntegerValue, ObligationId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerOverflowPolicy {
    Wrapping,   // Reduce the mathematical result into the carrier interval.
    Exact,      // Admitted only after Psi discharged the operation's range obligation.
    Saturating, // Clamp the mathematical result to the carrier interval.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerArithmeticOperator {
    Add,
    Subtract,
    Multiply,
    Divide,    // Quotient truncated toward zero; divisor is proven nonzero.
    Remainder, // Dividend-signed remainder; divisor is proven nonzero.
}

impl IntegerArithmeticOperator {
    pub const fn is_division(self) -> bool {
        matches!(self, Self::Divide | Self::Remainder)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerArithmetic {
    policy: IntegerOverflowPolicy,
    operator: IntegerArithmeticOperator,
    obligation: Option<ObligationId>,
}

impl IntegerArithmetic {
    pub const fn requires_obligation(
        policy: IntegerOverflowPolicy,
        operator: IntegerArithmeticOperator,
    ) -> bool {
        matches!(policy, IntegerOverflowPolicy::Exact) || operator.is_division()
    }

    pub const fn new(
        policy: IntegerOverflowPolicy,
        operator: IntegerArithmeticOperator,
        obligation: Option<ObligationId>,
    ) -> Option<Self> {
        if Self::requires_obligation(policy, operator) != obligation.is_some() {
            return None;
        }
        Some(Self {
            policy,
            operator,
            obligation,
        })
    }

    pub const fn policy(self) -> IntegerOverflowPolicy {
        self.policy
    }

    pub const fn operator(self) -> IntegerArithmeticOperator {
        self.operator
    }

    pub fn is(self, policy: IntegerOverflowPolicy, operator: IntegerArithmeticOperator) -> bool {
        self.policy == policy && self.operator == operator
    }

    pub const fn obligation(self) -> Option<ObligationId> {
        self.obligation
    }

    pub fn scalar_term(
        self,
        scalar_type: IntegerType,
        left: semantic_vocabulary::ScalarTerm,
        right: semantic_vocabulary::ScalarTerm,
    ) -> semantic_vocabulary::ScalarTerm {
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        use semantic_vocabulary::ScalarTerm as Term;
        let left = Box::new(left);
        let right = Box::new(right);
        match (self.policy, self.operator) {
            (Policy::Exact, Operator::Add) => Term::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            },
            (Policy::Exact, Operator::Subtract) => Term::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            },
            (Policy::Exact, Operator::Multiply) => Term::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            },
            (Policy::Exact, Operator::Divide) => Term::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            },
            (Policy::Exact, Operator::Remainder) => Term::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            },
            (Policy::Wrapping, Operator::Add) => Term::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            },
            (Policy::Wrapping, Operator::Subtract) => Term::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            },
            (Policy::Wrapping, Operator::Multiply) => Term::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            },
            (Policy::Wrapping, Operator::Divide) => Term::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            },
            (Policy::Wrapping, Operator::Remainder) => Term::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            },
            (Policy::Saturating, Operator::Add) => Term::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            },
            (Policy::Saturating, Operator::Subtract) => Term::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            },
            (Policy::Saturating, Operator::Multiply) => Term::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            },
            (Policy::Saturating, Operator::Divide) => Term::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            },
            (Policy::Saturating, Operator::Remainder) => Term::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            },
        }
    }

    pub fn evaluate(
        self,
        scalar_type: IntegerType,
        left: IntegerValue,
        right: IntegerValue,
    ) -> Option<IntegerValue> {
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        match (self.policy, self.operator) {
            (Policy::Wrapping, Operator::Add) => scalar_type.wrapping_add(left, right),
            (Policy::Exact, Operator::Add) => scalar_type.exact_add(left, right),
            (Policy::Saturating, Operator::Add) => scalar_type.saturating_add(left, right),
            (Policy::Wrapping, Operator::Subtract) => scalar_type.wrapping_sub(left, right),
            (Policy::Exact, Operator::Subtract) => scalar_type.exact_sub(left, right),
            (Policy::Saturating, Operator::Subtract) => scalar_type.saturating_sub(left, right),
            (Policy::Wrapping, Operator::Multiply) => scalar_type.wrapping_mul(left, right),
            (Policy::Exact, Operator::Multiply) => scalar_type.exact_mul(left, right),
            (Policy::Saturating, Operator::Multiply) => scalar_type.saturating_mul(left, right),
            (Policy::Wrapping, Operator::Divide) => scalar_type.wrapping_div(left, right),
            (Policy::Exact, Operator::Divide) => scalar_type.exact_div(left, right),
            (Policy::Saturating, Operator::Divide) => scalar_type.saturating_div(left, right),
            (Policy::Wrapping, Operator::Remainder) => scalar_type.wrapping_rem(left, right),
            (Policy::Exact, Operator::Remainder) => scalar_type.exact_rem(left, right),
            (Policy::Saturating, Operator::Remainder) => scalar_type.saturating_rem(left, right),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerShiftPolicy {
    Wrapping,
    Exact { obligation: ObligationId },
}

impl IntegerShiftPolicy {
    pub const fn obligation(self) -> Option<ObligationId> {
        match self {
            Self::Wrapping => None,
            Self::Exact { obligation } => Some(obligation),
        }
    }

    pub const fn overflow_policy(self) -> IntegerOverflowPolicy {
        match self {
            Self::Wrapping => IntegerOverflowPolicy::Wrapping,
            Self::Exact { .. } => IntegerOverflowPolicy::Exact,
        }
    }

    pub fn evaluate(
        self,
        direction: IntegerShiftDirection,
        value_type: IntegerType,
        value: IntegerValue,
        count_type: IntegerType,
        count: IntegerValue,
    ) -> Option<IntegerValue> {
        match (self, direction) {
            (Self::Wrapping, IntegerShiftDirection::Left) => {
                value_type.wrapping_shift_left(value, count_type, count)
            }
            (Self::Wrapping, IntegerShiftDirection::Right) => {
                value_type.wrapping_shift_right(value, count_type, count)
            }
            (Self::Exact { .. }, IntegerShiftDirection::Left) => {
                value_type.exact_shift_left(value, count_type, count)
            }
            (Self::Exact { .. }, IntegerShiftDirection::Right) => {
                value_type.exact_shift_right(value, count_type, count)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerShiftDirection {
    Left,
    Right,
}
