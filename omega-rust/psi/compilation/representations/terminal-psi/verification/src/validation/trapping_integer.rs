//! Independent checks for Trapping primitives and their operation-level
//! crash sites.
//!
//! A `TrappingInteger` operation carries only its primitive and operands.
//! Everything else the site needs is reconstructed here from the module:
//! the exact denotation (primitive, result carrier, operand carrier), the
//! fixed `Trap` cause, the normal-return result equation, and coverage by the
//! owning machine's published crash ceiling. No producer-supplied cause,
//! guard, or site row is trusted, so a changed operand type, a missing
//! ceiling, or a relocated site cannot pass.
//!
//! Formation requires a same-cause ceiling. An unconditional ceiling needs
//! no proof; guarded coverage is a crash-certificate question reconstructed
//! at the operation, before its normal-return result equation is available.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{IntegerType, Proposition, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{
    CrashCause, CrashRouteGuard, Operation, OperationKind, TerminalMachine,
    TrappingIntegerOperation, TrappingIntegerPrimitive,
};

use super::ModuleError;
use super::operations::require_defined;

/// The exact denotation of one validated Trapping primitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrappingIntegerDenotation {
    pub(crate) primitive: TrappingIntegerPrimitive,
    pub(crate) result_type: IntegerType,
    /// The right operand's carrier for binary arithmetic, the count's carrier
    /// for a shift, and the source carrier for a conversion.
    pub(crate) operand_type: IntegerType,
}

/// The cause every Trapping primitive commits. It is a property of the
/// policy, not a producer choice.
pub(crate) const TRAPPING_INTEGER_CAUSE: CrashCause = CrashCause::Trap;

fn fixed_integer(scalar_type: ScalarType) -> Option<IntegerType> {
    match scalar_type {
        ScalarType::Integer(integer) if !integer.is_address() => Some(integer),
        _ => None,
    }
}

/// Reconstruct the exact denotation from the operation's declared result and
/// operand types. Binary arithmetic reads two values of the result carrier;
/// a shift reads a value of the result carrier and an independently typed
/// fixed count; a conversion reads one fixed integer of any carrier. Address
/// carriers have no Trapping denotation.
pub(crate) fn trapping_integer_denotation(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<TrappingIntegerDenotation, ModuleError> {
    let OperationKind::TrappingInteger {
        operation: trapping,
    } = operation.kind
    else {
        unreachable!("dispatched a Trapping primitive")
    };
    let result_type = operation
        .result
        .scalar_ref()
        .and_then(|result| fixed_integer(result.scalar_type))
        .ok_or(ModuleError::TrappingIntegerRequiresFixedIntegerResult(
            operation.id,
        ))?;
    let operand_type = |value: ValueId| {
        value_types
            .get(&value)
            .copied()
            .and_then(fixed_integer)
            .ok_or(ModuleError::TrappingIntegerOperandTypeMismatch {
                operation: operation.id,
                operand: value,
            })
    };
    let primitive = trapping.primitive();
    let operand_type = match trapping {
        TrappingIntegerOperation::Add { left, right }
        | TrappingIntegerOperation::Subtract { left, right }
        | TrappingIntegerOperation::Multiply { left, right }
        | TrappingIntegerOperation::Divide { left, right }
        | TrappingIntegerOperation::Remainder { left, right } => {
            for operand in [left, right] {
                if operand_type(operand)? != result_type {
                    return Err(ModuleError::TrappingIntegerOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                    });
                }
            }
            result_type
        }
        TrappingIntegerOperation::ShiftLeft { value, count }
        | TrappingIntegerOperation::ShiftRight { value, count } => {
            if operand_type(value)? != result_type {
                return Err(ModuleError::TrappingIntegerOperandTypeMismatch {
                    operation: operation.id,
                    operand: value,
                });
            }
            operand_type(count)?
        }
        TrappingIntegerOperation::Convert { operand } => operand_type(operand)?,
    };
    Ok(TrappingIntegerDenotation {
        primitive,
        result_type,
        operand_type,
    })
}

/// Operand definedness and exact carrier agreement.
pub(super) fn validate_operands(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::TrappingInteger {
        operation: trapping,
    } = operation.kind
    else {
        unreachable!("dispatched a Trapping primitive")
    };
    for operand in trapping.operands() {
        require_defined(operand, value_types, defined)?;
    }
    trapping_integer_denotation(operation, value_types).map(|_| ())
}

/// The fact a normal return establishes: the result is the primitive's exact
/// mathematical value. It is sound only on the continuation, which a trap
/// never reaches, so it joins the path axioms after the operation exactly as
/// an Exact operation's result equation joins after its obligation.
pub(crate) fn normal_return_equation(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Proposition, ModuleError> {
    let exact = normal_return_expression(operation, value_types)?;
    let result = operation.result.expect_scalar();
    Ok(Proposition::Equal(
        ScalarTerm::value(result.id, result.scalar_type),
        exact,
    ))
}

fn normal_return_expression(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<ScalarTerm, ModuleError> {
    let denotation = trapping_integer_denotation(operation, value_types)?;
    let OperationKind::TrappingInteger {
        operation: trapping,
    } = operation.kind
    else {
        unreachable!("dispatched a Trapping primitive")
    };
    let term = |value: ValueId| ScalarTerm::value(value, value_types[&value]);
    let result_type = denotation.result_type;
    let exact = match trapping {
        TrappingIntegerOperation::Add { left, right } => {
            ScalarTerm::exact_integer_add(result_type, term(left), term(right))
        }
        TrappingIntegerOperation::Subtract { left, right } => {
            ScalarTerm::exact_integer_subtract(result_type, term(left), term(right))
        }
        TrappingIntegerOperation::Multiply { left, right } => {
            ScalarTerm::exact_integer_multiply(result_type, term(left), term(right))
        }
        TrappingIntegerOperation::Divide { left, right } => {
            ScalarTerm::exact_integer_divide(result_type, term(left), term(right))
        }
        TrappingIntegerOperation::Remainder { left, right } => {
            ScalarTerm::exact_integer_remainder(result_type, term(left), term(right))
        }
        TrappingIntegerOperation::ShiftLeft { value, count } => {
            ScalarTerm::exact_integer_shift_left(
                result_type,
                denotation.operand_type,
                term(value),
                term(count),
            )
        }
        TrappingIntegerOperation::ShiftRight { value, count } => {
            ScalarTerm::exact_integer_shift_right(
                result_type,
                denotation.operand_type,
                term(value),
                term(count),
            )
        }
        TrappingIntegerOperation::Convert { operand } => {
            ScalarTerm::integer_exact_cast(denotation.operand_type, result_type, term(operand))
        }
    }
    .map_err(ModuleError::MalformedProposition)?;
    Ok(exact)
}

/// Formation requires a same-cause bucket. Guarded coverage is checked by
/// independent crash-certificate replay, not granted by structural validation.
pub(super) fn validate_trapping_crash_sites(machine: &TerminalMachine) -> Result<(), ModuleError> {
    let covered = machine
        .contract
        .crash_routes
        .iter()
        .filter(|bucket| bucket.cause == TRAPPING_INTEGER_CAUSE)
        .any(|bucket| !bucket.alternatives.is_empty());
    for block in &machine.blocks {
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::TrappingInteger { .. }) && !covered {
                return Err(ModuleError::TrappingIntegerCrashUncovered {
                    machine: machine.id,
                    block: block.id,
                    operation: operation.id,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn has_unconditional_trap_ceiling(machine: &TerminalMachine) -> bool {
    machine.contract.crash_routes.iter().any(|bucket| {
        bucket.cause == TRAPPING_INTEGER_CAUSE
            && bucket.alternatives.contains(&CrashRouteGuard::Truth)
    })
}

/// Trapping and Exact have the same successful domain. Reuse the canonical
/// operation-local safety proposition; its complement is the intrinsic trap
/// predicate. This does not assert that the operation returns normally.
pub(crate) fn normal_return_condition(
    operation: &Operation,
    trapping: TrappingIntegerOperation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Proposition, ModuleError> {
    use terminal_semantics::CanonicalScalarGoal;
    let denotation = trapping_integer_denotation(operation, value_types)?;
    let term = |value: ValueId| ScalarTerm::value(value, value_types[&value]);
    let goal = match trapping {
        TrappingIntegerOperation::Convert { operand } => {
            CanonicalScalarGoal::ExactCastRepresentable {
                source_type: denotation.operand_type,
                target_type: denotation.result_type,
                operand: term(operand),
            }
        }
        TrappingIntegerOperation::Add { .. }
        | TrappingIntegerOperation::Subtract { .. }
        | TrappingIntegerOperation::Multiply { .. } => {
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: denotation.result_type,
                expression: normal_return_expression(operation, value_types)?,
            }
        }
        TrappingIntegerOperation::Divide { left, right }
        | TrappingIntegerOperation::Remainder { left, right } => {
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: denotation.result_type,
                left: term(left),
                right: term(right),
            }
        }
        TrappingIntegerOperation::ShiftLeft { value, count } => {
            CanonicalScalarGoal::ExactShiftLeftRepresentable {
                value_type: denotation.result_type,
                count_type: denotation.operand_type,
                value: term(value),
                count: term(count),
            }
        }
        TrappingIntegerOperation::ShiftRight { count, .. } => {
            CanonicalScalarGoal::ExactShiftCount {
                value_type: denotation.result_type,
                count_type: denotation.operand_type,
                count: term(count),
            }
        }
    };
    goal.kernel_proposition()
        .map_err(ModuleError::OperationSemanticSchema)
}

/// Division's exceptional operands have a direct equality meaning: zero,
/// and signed MIN with -1 for both quotient and remainder. Retain that
/// predicate rather than requiring a producer to derive equality from the
/// interval encoding used by Exact's successful-domain goal.
pub(crate) fn division_failure_condition(
    operation: &Operation,
    trapping: TrappingIntegerOperation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Option<Proposition>, ModuleError> {
    use semantic_vocabulary::{IntegerSign, IntegerValue};
    let (left, right) = match trapping {
        TrappingIntegerOperation::Divide { left, right }
        | TrappingIntegerOperation::Remainder { left, right } => (left, right),
        _ => return Ok(None),
    };
    let integer = trapping_integer_denotation(operation, value_types)?.result_type;
    let term = |value| ScalarTerm::value(value, ScalarType::Integer(integer));
    let literal =
        |value| ScalarTerm::integer(integer, value).map_err(ModuleError::MalformedProposition);
    let zero = match integer.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    };
    let zero_divisor = Proposition::Equal(term(right), literal(zero)?);
    Ok(Some(if integer.sign() == IntegerSign::Signed {
        Proposition::Disjunction(vec![
            zero_divisor,
            Proposition::Conjunction(vec![
                Proposition::Equal(term(left), literal(integer.minimum_value())?),
                Proposition::Equal(term(right), literal(IntegerValue::Signed(-1))?),
            ]),
        ])
    } else {
        zero_divisor
    }))
}
