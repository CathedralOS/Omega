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
//! Coverage is the crash-terminator rule with an empty retained guard: a
//! same-cause bucket covers the site only when it is unconditional. A guarded
//! `Trap` ceiling needs the operation's incoming conjunction as evidence,
//! which this representation does not retain yet, so such a machine rejects
//! instead of being treated as covered.

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
    let denotation = trapping_integer_denotation(operation, value_types)?;
    let OperationKind::TrappingInteger {
        operation: trapping,
    } = operation.kind
    else {
        unreachable!("dispatched a Trapping primitive")
    };
    let result = operation.result.expect_scalar();
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
    Ok(Proposition::Equal(
        ScalarTerm::value(result.id, result.scalar_type),
        exact,
    ))
}

/// Every Trapping primitive's site must be covered by an unconditional
/// same-cause bucket of its machine's published ceiling.
pub(super) fn validate_trapping_crash_sites(machine: &TerminalMachine) -> Result<(), ModuleError> {
    let covered = machine
        .contract
        .crash_routes
        .iter()
        .filter(|bucket| bucket.cause == TRAPPING_INTEGER_CAUSE)
        .any(|bucket| bucket.alternatives == [CrashRouteGuard::Truth]);
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
