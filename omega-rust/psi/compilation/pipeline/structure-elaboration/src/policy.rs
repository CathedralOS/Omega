/*
What an arithmetic or shift operator means under a policy, decided once for
executable code and for contract terms.

numeric_values.md, "Policy operators": "`a +% b` on two `u32` values is
exactly what `+` means for two `u32 in Wrapping` operands, and likewise for
every form." domains.md, "Operands": "The rule is the same in executable
code, `requires` and `ensures`." So a body and a clause ask the same table
which Terminal operation an operator is under a policy (`total`), and a
clause's term is the value Terminal semantics says that operation computes
(`term`). Neither reader has a meaning of its own for a policy operator.
*/

use std::collections::BTreeMap;

use numerics::arithmetic::ArithmeticDomain;
use semantic_vocabulary::{IntegerType, OperationId, Proposition, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};
use typed_trees::expression::BinaryOperator;

use crate::body::{Policy, Strict};

/// What the suffix written on one operator selects.
pub(crate) enum Suffix {
    /// No suffix: the operator means what its operands' types say.
    Unwritten,
    /// `%` or `|`: the operation under that policy, at this one node.
    Total(Policy),
    /// `!`: the operation that crashes where it would overflow. It can
    /// transfer control, and no type on this route carries `Trapping`.
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

/// The Terminal operation `operation` is under `policy` when that operation
/// owes nothing: an add, subtract or multiply under either policy, and a
/// Wrapping shift. `None` for the rest: a divide or remainder "still owes its
/// divisor" under any policy, so lowering attaches that obligation itself,
/// and no other operation has a policy form.
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

/// The value of `left <operation> right` under `policy` as a term, when the
/// operation is total: the right side of the result equation Terminal
/// semantics states for the operation `total` names, with each operand's
/// term in place of the operand.
///
/// contracts.md, "Total arithmetic": "Every term in a contract, domain
/// predicate, or crash guard is total." An operation with an obligation or a
/// crash has no such equation here, so it forms no term (`None`).
pub(crate) fn term(
    operation: Strict,
    policy: Policy,
    carrier: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
) -> Option<ScalarTerm> {
    // The operation is stated over two operand values and its result, then
    // the operands are replaced; the three identities name nothing else.
    let [left_value, right_value, result] = [1, 2, 3].map(ValueId::new);
    let (left_value, right_value, result) = (left_value?, right_value?, result?);
    let kind = total(operation, policy, left_value, right_value)?;
    let types = BTreeMap::from([
        (left_value, left.scalar_type()),
        (right_value, right.scalar_type()),
    ]);
    let stated = Operation::unsuspended(
        OperationId::new(1)?,
        OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: ScalarType::Integer(carrier),
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        }),
        kind,
    );
    let semantics = terminal_semantics::goal_free_scalar_leaf_semantics(&stated, &types)
        .ok()
        .flatten()?;
    let operands = BTreeMap::from([(left_value, left), (right_value, right)]);
    match terminal_semantics::substitution::substitute_proposition_values(
        semantics.result_equation(),
        &operands,
    ) {
        Proposition::Equal(_, value) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerValue};

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
            term(Strict::Add, Policy::Wrapping, carrier, formal(2), formal(1)),
            ScalarTerm::wrapping_integer_add(carrier, formal(2), formal(1)).ok(),
        );
        assert_eq!(
            term(
                Strict::Subtract,
                Policy::Saturating,
                carrier,
                formal(1),
                seven.clone()
            ),
            ScalarTerm::saturating_integer_subtract(carrier, formal(1), seven).ok(),
        );
    }

    /// Only an operation that owes nothing forms a term: a divide or a
    /// remainder owes its divisor under either policy, a Saturating shift is
    /// no operation, and a comparison has no policy form.
    #[test]
    fn an_operation_that_owes_a_condition_or_has_no_policy_form_is_no_term() {
        let carrier = u32_type();
        for policy in [Policy::Wrapping, Policy::Saturating] {
            for operation in [Strict::Add, Strict::Subtract, Strict::Multiply] {
                assert!(
                    term(operation, policy, carrier, formal(1), formal(2)).is_some(),
                    "{operation:?} under {policy:?}"
                );
            }
            for operation in [Strict::Divide, Strict::Remainder, Strict::Less] {
                assert_eq!(
                    term(operation, policy, carrier, formal(1), formal(2)),
                    None,
                    "{operation:?} under {policy:?}"
                );
            }
        }
        assert!(
            term(
                Strict::ShiftLeft,
                Policy::Wrapping,
                carrier,
                formal(1),
                formal(2)
            )
            .is_some()
        );
        assert_eq!(
            term(
                Strict::ShiftLeft,
                Policy::Saturating,
                carrier,
                formal(1),
                formal(2)
            ),
            None
        );
    }
}
