/*
A contract must mean the same arithmetic as executable code. Operand selection
is shared by both readers; this owner maps the selected operation to canonical
scalar computations and executable operations. A contract computation keeps
its own formation goal, so even Wrapping or Saturating division still requires
a nonzero divisor before the containing clause can become a fact.

numeric_values.md, "Policy operators", makes suffixed operations the same
operation as the corresponding policy-qualified operands. domains.md,
"Operands", applies that selection in code, requires and ensures alike.
*/

use numerics::arithmetic::ArithmeticDomain;
use semantic_vocabulary::{IntegerType, ScalarTerm, ScalarType, ValueId};
use terminal_psi::OperationKind;
use typed_trees::expression::BinaryOperator;

use crate::body::{Policy, Strict, ValueType};

/// What the suffix written on one operator selects.
pub(crate) enum Suffix {
    /// No suffix: the operator means what its operands' types say.
    Unwritten,
    /// `%` or `|`: the operation under that policy, at this one node.
    Total(Policy),
    /// `!`: the operation that crashes where it would overflow. It can
    /// transfer control, so it is never a term.
    Trapping,
}

pub(crate) fn suffix(written: ArithmeticDomain) -> Suffix {
    match written {
        ArithmeticDomain::Exact => Suffix::Unwritten,
        ArithmeticDomain::Wrapping => Suffix::Total(Policy::Wrapping),
        ArithmeticDomain::Saturating => Suffix::Total(Policy::Saturating),
        ArithmeticDomain::Trapping => Suffix::Trapping,
    }
}

/// The operation a suffix may be written on: arithmetic and the two shifts.
/// A comparison, a logical or a bitwise operator has no policy form.
pub(crate) fn suffixed(operator: BinaryOperator) -> Result<Strict, &'static str> {
    match operator {
        BinaryOperator::Add => Ok(Strict::Add),
        BinaryOperator::Subtract => Ok(Strict::Subtract),
        BinaryOperator::Multiply => Ok(Strict::Multiply),
        BinaryOperator::Divide => Ok(Strict::Divide),
        BinaryOperator::Modulo => Ok(Strict::Remainder),
        BinaryOperator::ShiftLeft => Ok(Strict::ShiftLeft),
        BinaryOperator::ShiftRight => Ok(Strict::ShiftRight),
        BinaryOperator::And
        | BinaryOperator::Or
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::CaseMembership => {
            Err("policy suffix on an operator that is neither arithmetic nor a shift")
        }
    }
}

/// "A wholly anonymous tree rejects, because it has no carrier to wrap, clamp
/// or trap at."
pub(crate) const WHOLLY_ANONYMOUS: &str = "policy operator on a wholly anonymous tree";

/// "Both operands are unqualified."
pub(crate) const QUALIFIED_OPERAND: &str =
    "policy operator on an operand that is not an unqualified integer";

/// Whether `operation` takes a count beside its value: the count "keeps its
/// own" type and is never the operand a policy is matched against.
pub(crate) const fn shifts(operation: Strict) -> bool {
    matches!(operation, Strict::ShiftLeft | Strict::ShiftRight)
}

/// Both the contract and executable forms of complement retain the exact
/// operand type. Only a bare carrier or one core policy has this operation.
pub(crate) fn bitwise_not_type(operand: ValueType) -> Option<ValueType> {
    match operand {
        ValueType::Scalar(ScalarType::Integer(integer), qualification)
            if !integer.is_address() && qualification.domains.is_empty() =>
        {
            Some(operand)
        }
        _ => None,
    }
}

/// The Terminal operation `operation` is under `policy` when that operation
/// owes nothing: an add, subtract or multiply under Wrapping or Saturating,
/// and a Wrapping shift. `None` for the rest: a divide or remainder "still
/// owes its divisor" under any policy, so lowering attaches that obligation
/// itself, a Trapping operation is a crash site, and no other operation has
/// a policy form.
pub(crate) fn total(
    operation: Strict,
    policy: Policy,
    left: ValueId,
    right: ValueId,
) -> Option<OperationKind> {
    Some(match (operation, policy) {
        (Strict::Add, Policy::Wrapping) => OperationKind::WrappingIntegerAdd { left, right },
        (Strict::Add, Policy::Saturating) => OperationKind::SaturatingIntegerAdd { left, right },
        (Strict::Subtract, Policy::Wrapping) => {
            OperationKind::WrappingIntegerSubtract { left, right }
        }
        (Strict::Subtract, Policy::Saturating) => {
            OperationKind::SaturatingIntegerSubtract { left, right }
        }
        (Strict::Multiply, Policy::Wrapping) => {
            OperationKind::WrappingIntegerMultiply { left, right }
        }
        (Strict::Multiply, Policy::Saturating) => {
            OperationKind::SaturatingIntegerMultiply { left, right }
        }
        (Strict::ShiftLeft, Policy::Wrapping) => OperationKind::WrappingIntegerShiftLeft {
            value: left,
            count: right,
        },
        (Strict::ShiftRight, Policy::Wrapping) => OperationKind::WrappingIntegerShiftRight {
            value: left,
            count: right,
        },
        (_, Policy::Trapping) => return None,
        (
            Strict::Divide
            | Strict::Remainder
            | Strict::ShiftLeft
            | Strict::ShiftRight
            | Strict::BitwiseAnd
            | Strict::BitwiseOr
            | Strict::BitwiseXor
            | Strict::Equal
            | Strict::NotEqual
            | Strict::Less
            | Strict::LessOrEqual
            | Strict::Greater
            | Strict::GreaterOrEqual,
            _,
        ) => return None,
    })
}

/// Contract arithmetic retains its selected canonical computation. Whether
/// that computation is total is decided later by declaration formation,
/// using CanonicalScalarGoal; an overflow policy does not erase divisor goals.
pub(crate) fn term(
    operation: Strict,
    policy: Option<Policy>,
    carrier: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
) -> Option<ScalarTerm> {
    if shifts(operation) {
        let count = right.scalar_type().as_integer()?;
        let constructor = match (operation, policy) {
            (Strict::ShiftLeft, None) => ScalarTerm::exact_integer_shift_left,
            (Strict::ShiftRight, None | Some(Policy::Saturating)) => {
                ScalarTerm::exact_integer_shift_right
            }
            (Strict::ShiftLeft, Some(Policy::Wrapping)) => ScalarTerm::wrapping_integer_shift_left,
            (Strict::ShiftRight, Some(Policy::Wrapping)) => {
                ScalarTerm::wrapping_integer_shift_right
            }
            _ => return None,
        };
        return constructor(carrier, count, left, right).ok();
    }
    let constructor = match (operation, policy) {
        (Strict::Add, None) => ScalarTerm::exact_integer_add,
        (Strict::Subtract, None) => ScalarTerm::exact_integer_subtract,
        (Strict::Multiply, None) => ScalarTerm::exact_integer_multiply,
        (Strict::Divide, None) => ScalarTerm::exact_integer_divide,
        (Strict::Remainder, None) => ScalarTerm::exact_integer_remainder,
        (Strict::Add, Some(Policy::Wrapping)) => ScalarTerm::wrapping_integer_add,
        (Strict::Subtract, Some(Policy::Wrapping)) => ScalarTerm::wrapping_integer_subtract,
        (Strict::Multiply, Some(Policy::Wrapping)) => ScalarTerm::wrapping_integer_multiply,
        (Strict::Divide, Some(Policy::Wrapping)) => ScalarTerm::wrapping_integer_divide,
        (Strict::Remainder, Some(Policy::Wrapping)) => ScalarTerm::wrapping_integer_remainder,
        (Strict::Add, Some(Policy::Saturating)) => ScalarTerm::saturating_integer_add,
        (Strict::Subtract, Some(Policy::Saturating)) => ScalarTerm::saturating_integer_subtract,
        (Strict::Multiply, Some(Policy::Saturating)) => ScalarTerm::saturating_integer_multiply,
        (Strict::Divide, Some(Policy::Saturating)) => ScalarTerm::saturating_integer_divide,
        (Strict::Remainder, Some(Policy::Saturating)) => ScalarTerm::saturating_integer_remainder,
        (Strict::BitwiseAnd, None) => ScalarTerm::integer_bitwise_and,
        (Strict::BitwiseOr, None) => ScalarTerm::integer_bitwise_or,
        (Strict::BitwiseXor, None) => ScalarTerm::integer_bitwise_xor,
        _ => return None,
    };
    constructor(carrier, left, right).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

    fn u32_type() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
    }

    fn formal(position: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(position).unwrap(),
            ScalarType::Integer(u32_type()),
        )
    }

    /// A clause's term for a total policy operation is the operation over
    /// the clause's own operands, whatever identities they have: the two
    /// formals swapped, and a literal, stay where the clause wrote them.
    #[test]
    fn a_total_policy_operation_is_a_term_over_the_operands_given() {
        let carrier = u32_type();
        let seven = ScalarTerm::integer(carrier, IntegerValue::Unsigned(7)).unwrap();
        assert_eq!(
            term(
                Strict::Add,
                Some(Policy::Wrapping),
                carrier,
                formal(2),
                formal(1)
            ),
            ScalarTerm::wrapping_integer_add(carrier, formal(2), formal(1)).ok(),
        );
        assert_eq!(
            term(
                Strict::Subtract,
                Some(Policy::Saturating),
                carrier,
                formal(1),
                seven.clone()
            ),
            ScalarTerm::saturating_integer_subtract(carrier, formal(1), seven).ok(),
        );
    }

    /// A policy supplies its exact computation, but supplies no operation
    /// for a Saturating left shift or a policy-qualified comparison.
    #[test]
    fn an_operation_without_a_policy_meaning_is_no_term() {
        let carrier = u32_type();
        for policy in [Policy::Wrapping, Policy::Saturating] {
            for operation in [Strict::Add, Strict::Subtract, Strict::Multiply] {
                assert!(
                    term(operation, Some(policy), carrier, formal(1), formal(2)).is_some(),
                    "{operation:?} under {policy:?}"
                );
            }
            assert_eq!(
                term(Strict::Less, Some(policy), carrier, formal(1), formal(2)),
                None
            );
        }
        assert!(
            term(
                Strict::ShiftLeft,
                Some(Policy::Wrapping),
                carrier,
                formal(1),
                formal(2)
            )
            .is_some()
        );
        assert_eq!(
            term(
                Strict::ShiftLeft,
                Some(Policy::Saturating),
                carrier,
                formal(1),
                formal(2)
            ),
            None
        );
    }

    #[test]
    fn contract_division_keeps_its_definedness_goal_under_each_policy() {
        for policy in [None, Some(Policy::Wrapping), Some(Policy::Saturating)] {
            for operation in [Strict::Divide, Strict::Remainder] {
                let term = term(operation, policy, u32_type(), formal(1), formal(2)).unwrap();
                let goal = terminal_semantics::CanonicalScalarGoal::for_term(&term)
                    .unwrap()
                    .unwrap();
                assert!(goal.kernel_proposition().is_ok());
            }
        }
    }
}
