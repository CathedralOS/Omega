//! Independent denotation audit of the completed overflow-eligibility circuit.
//! Integer images supply exact IEEE exponent and magnitude predicates; Bool
//! images supply conjunction of normalized truth values. The recovered semantic
//! predicates must equal the original call's ordered numeric input conditions.
//! This test/artifact-review owner never reconstructs the emitter's operations.

use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerValue, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind};

#[derive(Debug, PartialEq, Eq)]
enum Predicate {
    Finite(ValueId, IeeeFloatFormat),
    Nonzero(ValueId, IeeeFloatFormat),
}

pub(super) fn verify(
    operations: &[Operation],
    eligible: ValueId,
    arguments: &[ValueId],
    types: &[ScalarType],
    divisor: Option<usize>,
) -> bool {
    if arguments.len() != types.len() {
        return false;
    }
    let mut expected = Vec::new();
    for (position, (argument, kind)) in arguments.iter().zip(types).enumerate() {
        match kind {
            ScalarType::IeeeFloat(format) => {
                expected.push(Predicate::Finite(*argument, *format));
                if divisor == Some(position) {
                    expected.push(Predicate::Nonzero(*argument, *format));
                }
            }
            ScalarType::Integer(_) if divisor != Some(position) => {}
            _ => return false,
        }
    }
    if divisor.is_some_and(|position| position >= arguments.len()) {
        return false;
    }
    let mut budget = 4096;
    let Some(root) = definition(operations, eligible, &mut budget) else {
        return false;
    };
    if root
        .result
        .scalar_ref()
        .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
    {
        return false;
    }
    let OperationKind::IntegerEqual { left, right } = root.kind else {
        return false;
    };
    let mut observed = Vec::new();
    let conjunction = if is_byte_one(operations, right, &mut budget) {
        left
    } else if is_byte_one(operations, left, &mut budget) {
        right
    } else {
        return false;
    };
    byte_predicates(operations, conjunction, &mut budget, &mut observed) && observed == expected
}

fn definition<'a>(
    operations: &'a [Operation],
    value: ValueId,
    budget: &mut usize,
) -> Option<&'a Operation> {
    *budget = budget.checked_sub(1)?;
    let mut matches = operations.iter().filter(|operation| {
        operation
            .result
            .scalar_ref()
            .is_some_and(|result| result.id == value)
    });
    let operation = matches.next()?;
    matches.next().is_none().then_some(operation)
}
fn unsigned_constant(
    operations: &[Operation],
    value: ValueId,
    bits: u16,
    budget: &mut usize,
) -> Option<u128> {
    let operation = definition(operations, value, budget)?;
    let ScalarType::Integer(integer) = operation.result.scalar_ref()?.scalar_type else {
        return None;
    };
    if integer.sign() != IntegerSign::Unsigned || integer.bits() != bits {
        return None;
    }
    let OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(value),
    } = operation.kind
    else {
        return None;
    };
    Some(value)
}
fn is_byte_one(operations: &[Operation], value: ValueId, budget: &mut usize) -> bool {
    unsigned_constant(operations, value, 8, budget) == Some(1)
}
fn byte_predicates(
    operations: &[Operation],
    value: ValueId,
    budget: &mut usize,
    predicates: &mut Vec<Predicate>,
) -> bool {
    if is_byte_one(operations, value, budget) {
        return true;
    }
    let Some(operation) = definition(operations, value, budget) else {
        return false;
    };
    let Some(result) = operation.result.scalar_ref() else {
        return false;
    };
    if !matches!(result.scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8)
    {
        return false;
    }
    match operation.kind {
        OperationKind::IntegerBitwiseAnd { left, right } => {
            byte_predicates(operations, left, budget, predicates)
                && byte_predicates(operations, right, budget, predicates)
        }
        OperationKind::ScalarRepresentationView {
            operand,
            landing_obligation: None,
        } => {
            let Some(predicate) = predicate(operations, operand, budget) else {
                return false;
            };
            predicates.push(predicate);
            true
        }
        _ => false,
    }
}
fn predicate(operations: &[Operation], value: ValueId, budget: &mut usize) -> Option<Predicate> {
    let negation = definition(operations, value, budget)?;
    if negation.result.scalar_ref()?.scalar_type != ScalarType::Boolean {
        return None;
    }
    let OperationKind::BooleanNot { operand } = negation.kind else {
        return None;
    };
    let comparison = definition(operations, operand, budget)?;
    if comparison.result.scalar_ref()?.scalar_type != ScalarType::Boolean {
        return None;
    }
    let OperationKind::IntegerEqual { left, right } = comparison.kind else {
        return None;
    };
    image_comparison(operations, left, right, budget)
        .or_else(|| image_comparison(operations, right, left, budget))
}
fn image_comparison(
    operations: &[Operation],
    image: ValueId,
    comparison: ValueId,
    budget: &mut usize,
) -> Option<Predicate> {
    let masked = definition(operations, image, budget)?;
    let ScalarType::Integer(integer) = masked.result.scalar_ref()?.scalar_type else {
        return None;
    };
    if integer.sign() != IntegerSign::Unsigned {
        return None;
    }
    let (format, exponent, magnitude) = match integer.bits() {
        32 => (IeeeFloatFormat::Binary32, 0x7f80_0000, 0x7fff_ffff),
        64 => (
            IeeeFloatFormat::Binary64,
            0x7ff0_0000_0000_0000,
            0x7fff_ffff_ffff_ffff,
        ),
        _ => return None,
    };
    let comparison = unsigned_constant(operations, comparison, integer.bits(), budget)?;
    let OperationKind::IntegerBitwiseAnd { left, right } = masked.kind else {
        return None;
    };
    let (view, mask) =
        if let Some(mask) = unsigned_constant(operations, right, integer.bits(), budget) {
            (left, mask)
        } else {
            (
                right,
                unsigned_constant(operations, left, integer.bits(), budget)?,
            )
        };
    let view = definition(operations, view, budget)?;
    if view.result.scalar_ref()?.scalar_type != ScalarType::Integer(integer) {
        return None;
    }
    let OperationKind::ScalarRepresentationView {
        operand,
        landing_obligation: None,
    } = view.kind
    else {
        return None;
    };
    if mask == exponent && comparison == exponent {
        Some(Predicate::Finite(operand, format))
    } else if mask == magnitude && comparison == 0 {
        Some(Predicate::Nonzero(operand, format))
    } else {
        None
    }
}
