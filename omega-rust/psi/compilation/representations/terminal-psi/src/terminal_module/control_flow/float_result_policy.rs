//! Arithmetic policy on an already evaluated IEEE result.
//!
//! The selected boundary still owns the arithmetic and its provider identity.
//! This operation consumes that call's raw result exactly once, then publishes
//! the payload the authored consumer sees. Policy never rewrites the call or
//! re-evaluates its operands. All payload operands have the result's IEEE format.
//!
//! Trapping belongs to this operation's own `Trap` site, like Trapping integer
//! arithmetic. A manufactured branch/Crash edge would lose that denotation;
//! manufactured authored comparisons would lose source-selection custody.
//! Saturation needs the original operands as well as the rounded result:
//! propagated infinity and division by signed zero are not magnitude overflow.
//! Source replay must independently bind those operands and the division rule
//! to the original checked application, not trust the producer's classification.

use semantic_vocabulary::ValueId;

/// Whether a binary operation has the division-by-zero exception to overflow
/// saturation. This is semantic operation data, not an ambient arithmetic mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatSaturationRule {
    /// Clamp infinity produced from two finite operands to signed maximum.
    FiniteOperandOverflow,
    /// The same rule, except either signed-zero divisor preserves raw infinity.
    FiniteDivisionOverflow,
}

/// A result adapter, separate from the boundary operation that produced it.
/// The current saturation carrier is binary; other arities need their own
/// exact operand custody before they can use this operation. Trapping depends
/// only on the semantic result and is independent of the call's arity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatResultPolicy {
    /// Preserve finite values; crash with cause `Trap` for any NaN or infinity.
    Trapping { raw_result: ValueId },
    /// Preserve the raw result except for the finite-operand magnitude overflow
    /// selected by `rule`. NaNs, signed zeros and finite underflow stay intact.
    Saturating {
        raw_result: ValueId,
        left: ValueId,
        right: ValueId,
        rule: FloatSaturationRule,
    },
}

impl FloatResultPolicy {
    pub const fn raw_result(self) -> ValueId {
        match self {
            Self::Trapping { raw_result } | Self::Saturating { raw_result, .. } => raw_result,
        }
    }

    pub const fn can_trap(self) -> bool {
        matches!(self, Self::Trapping { .. })
    }

    /// Values needed after the original call: its original inputs, then result.
    pub fn operands(self) -> Vec<ValueId> {
        match self {
            Self::Trapping { raw_result } => vec![raw_result],
            Self::Saturating {
                raw_result,
                left,
                right,
                ..
            } => vec![left, right, raw_result],
        }
    }

    pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Trapping { raw_result } => *raw_result = map(*raw_result),
            Self::Saturating {
                raw_result,
                left,
                right,
                ..
            } => {
                *left = map(*left);
                *right = map(*right);
                *raw_result = map(*raw_result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FloatResultPolicy, FloatSaturationRule, ValueId};

    #[test]
    fn operand_mapping_preserves_policy_and_divisor_position() {
        let value = |raw| ValueId::new(raw).unwrap();
        for rule in [
            FloatSaturationRule::FiniteOperandOverflow,
            FloatSaturationRule::FiniteDivisionOverflow,
        ] {
            let mut policy = FloatResultPolicy::Saturating {
                raw_result: value(3),
                left: value(1),
                right: value(2),
                rule,
            };
            assert!(!policy.can_trap());
            assert_eq!(policy.operands(), [value(1), value(2), value(3)]);
            policy.map_operands(&mut |operand| value(operand.get() + 10));
            assert_eq!(
                policy,
                FloatResultPolicy::Saturating {
                    raw_result: value(13),
                    left: value(11),
                    right: value(12),
                    rule,
                }
            );
        }
        let mut policy = FloatResultPolicy::Trapping {
            raw_result: value(3),
        };
        assert!(policy.can_trap());
        policy.map_operands(&mut |operand| value(operand.get() + 10));
        assert_eq!(policy.operands(), [value(13)]);
        assert_eq!(policy.raw_result(), value(13));
    }
}
