//! Fixed-width integer arithmetic and shift vocabulary: one overflow policy
//! and one operator per operation, instead of one operation per pair.
//!
//! # Why the policy and the operator are separate fields
//!
//! Terminal Psi names every admitted pair as its own operation
//! (`WrappingIntegerAdd`, `ExactIntegerDivide`, ...). Omega used to mirror
//! that cross product as nineteen `AbstractOperation` variants and nineteen
//! `TargetIntegerExpression` variants, so every consumer repeated fifteen to
//! nineteen arms that differed only in which pair they named. The pair is
//! now data: consumers read the policy and the operator and match only where
//! the pair changes behavior.
//!
//! # Where the obligation lives
//!
//! The admitted pairs do not form a full product with one obligation shape.
//! Add, subtract and multiply carry a verifier-discharged range obligation
//! only under `Exact`; divide and remainder carry a nonzero-divisor
//! obligation under every policy; shifts exist only as `Wrapping` and
//! `Exact`, and only `Exact` carries one. [`IntegerArithmetic`] keeps its
//! fields private so [`IntegerArithmetic::new`] can refuse an obligation that
//! is missing or extra for its pair; [`IntegerShiftPolicy`] carries the shift
//! obligation inside its `Exact` arm, so a wrapping shift cannot hold one.
//!
//! Identity encodings key on the pair, not on this layout: each owner maps
//! every pair to the stable tag its former variant used.

use semantic_vocabulary::{IntegerType, IntegerValue, ObligationId};

/// How one fixed-width integer operation treats a mathematical result that
/// its carrier cannot represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerOverflowPolicy {
    /// Reduce the mathematical result into the carrier interval.
    Wrapping,
    /// Admitted only after Psi discharged the operation's range obligation.
    /// Target realization may share the wrapping instruction, but the
    /// semantic identity remains distinct for optimization and audit.
    Exact,
    /// Clamp the mathematical result to the carrier interval.
    Saturating,
}

/// The binary arithmetic operator of one [`IntegerArithmetic`] operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerArithmeticOperator {
    Add,
    Subtract,
    Multiply,
    /// Quotient truncated toward zero; the divisor is proven nonzero.
    Divide,
    /// Dividend-signed remainder; the divisor is proven nonzero.
    Remainder,
}

impl IntegerArithmeticOperator {
    /// Divide and remainder carry a nonzero-divisor obligation under every
    /// overflow policy.
    pub const fn is_division(self) -> bool {
        matches!(self, Self::Divide | Self::Remainder)
    }
}

/// One admitted policy and operator pair, with the verifier-discharged
/// obligation the pair requires. The obligation is present exactly under the
/// `Exact` policy and for every divide or remainder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerArithmetic {
    policy: IntegerOverflowPolicy,
    operator: IntegerArithmeticOperator,
    obligation: Option<ObligationId>,
}

impl IntegerArithmetic {
    /// Whether the pair is admitted only with a verifier-discharged
    /// obligation.
    pub const fn requires_obligation(
        policy: IntegerOverflowPolicy,
        operator: IntegerArithmeticOperator,
    ) -> bool {
        matches!(policy, IntegerOverflowPolicy::Exact) || operator.is_division()
    }

    /// The pair with its obligation, or `None` when the obligation is
    /// missing for a pair that requires one or present for a pair that has
    /// none.
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

    /// Whether this operation is exactly the `policy` `operator` pair; rules
    /// that own one pair select it with this.
    pub fn is(self, policy: IntegerOverflowPolicy, operator: IntegerArithmeticOperator) -> bool {
        self.policy == policy && self.operator == operator
    }

    /// The range or nonzero-divisor obligation, present exactly when
    /// [`Self::requires_obligation`] holds for this pair.
    pub const fn obligation(self) -> Option<ObligationId> {
        self.obligation
    }

    /// Denotation after successful execution. Exact arithmetic's existing
    /// range obligation still governs admission; this records no new proof.
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

    /// The exact language result over two admitted operands of
    /// `scalar_type`, or `None` when the pair has no value for them (an
    /// unrepresentable exact result or a zero divisor).
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

/// The admitted shift policies. A wrapping shift reduces its count modulo
/// the value width; an exact shift carries the verifier-discharged obligation
/// that its count is inside `[0, width)` and, for a left shift, that the
/// shifted value is representable. There is no saturating shift.
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

    /// The overflow policy this shift policy names.
    pub const fn overflow_policy(self) -> IntegerOverflowPolicy {
        match self {
            Self::Wrapping => IntegerOverflowPolicy::Wrapping,
            Self::Exact { .. } => IntegerOverflowPolicy::Exact,
        }
    }

    /// The exact language result of shifting an admitted `value_type` value
    /// by an admitted `count_type` count, or `None` when the policy has no
    /// value for them.
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

/// A right shift is arithmetic for signed values and logical for unsigned
/// values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerShiftDirection {
    Left,
    Right,
}
