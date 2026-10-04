//! Validate an IEEE result adapter independently of its arithmetic producer.
//!
//! The adapter consumes already evaluated values. Its result format determines
//! every operand format; no implicit conversion or reinterpretation is allowed.
//! Source replay separately binds these identities and the saturation rule to
//! the checked call. Terminal formation cannot infer that source relationship.
//!
//! A Trapping adapter owns an operation-level Trap site. Its failure predicate
//! is the complement of the open interval (-infinity, +infinity), using IEEE
//! comparisons so that unordered NaNs fail too. These are verifier propositions,
//! not invented authored comparison operations. The normal continuation alone
//! learns that the preserved payload is finite.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue, Proposition, ScalarTerm, ScalarType,
    ValueId,
};
use terminal_psi::{CrashCause, FloatResultPolicy, Operation, OperationKind, TerminalMachine};

use super::ModuleError;
use super::operations::require_defined;

pub(crate) fn format(operation: &Operation) -> Result<IeeeFloatFormat, ModuleError> {
    match operation
        .result
        .scalar_ref()
        .map(|result| result.scalar_type)
    {
        Some(ScalarType::IeeeFloat(format)) => Ok(format),
        _ => Err(ModuleError::FloatResultPolicyRequiresFloatResult(
            operation.id,
        )),
    }
}

pub(super) fn validate_operands(
    operation: &Operation,
    policy: FloatResultPolicy,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let expected = ScalarType::IeeeFloat(format(operation)?);
    for operand in policy.operands() {
        require_defined(operand, value_types, defined)?;
        if value_types.get(&operand) != Some(&expected) {
            return Err(ModuleError::FloatResultPolicyOperandTypeMismatch {
                operation: operation.id,
                operand,
                expected,
            });
        }
    }
    Ok(())
}

/// Structural formation requires a same-cause route. Guarded routes still owe
/// their coverage proof under the operation's independently reconstructed failure
/// predicate; a declared guard is never assumed at the site.
pub(super) fn validate_crash_sites(machine: &TerminalMachine) -> Result<(), ModuleError> {
    let has_route = machine
        .contract
        .crash_routes
        .iter()
        .any(|route| route.cause == CrashCause::Trap && !route.alternatives.is_empty());
    for block in &machine.blocks {
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::FloatResultPolicy { policy } if policy.can_trap())
                && !has_route
            {
                return Err(ModuleError::FloatResultPolicyCrashUncovered {
                    machine: machine.id,
                    block: block.id,
                    operation: operation.id,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn failure_condition(
    operation: &Operation,
    raw_result: ValueId,
) -> Result<Proposition, ModuleError> {
    let format = format(operation)?;
    Ok(Proposition::Disjunction(finite_bounds(
        format,
        raw_result,
        IeeeFloatComparisonKind::NotLessThan,
    )))
}

pub(crate) fn normal_return_facts(
    operation: &Operation,
    raw_result: ValueId,
) -> Result<Vec<Proposition>, ModuleError> {
    let format = format(operation)?;
    let result =
        operation
            .result
            .scalar_ref()
            .ok_or(ModuleError::FloatResultPolicyRequiresFloatResult(
                operation.id,
            ))?;
    let mut facts = finite_bounds(format, result.id, IeeeFloatComparisonKind::LessThan);
    facts.push(Proposition::Equal(
        ScalarTerm::value(result.id, result.scalar_type),
        ScalarTerm::value(raw_result, result.scalar_type),
    ));
    Ok(facts)
}

/// Saturation has a conditional range, independently of who produced the raw
/// payload. A raw NaN stays NaN. Finite division by zero preserves infinity.
/// The arithmetic producer's normal contract must establish those premises.
pub(crate) fn saturating_return_facts(
    operation: &Operation,
    policy: FloatResultPolicy,
) -> Result<Vec<Proposition>, ModuleError> {
    let FloatResultPolicy::Saturating {
        raw_result,
        left,
        right,
        rule,
    } = policy
    else {
        return Ok(Vec::new());
    };
    let format = format(operation)?;
    let scalar_type = ScalarType::IeeeFloat(format);
    let result =
        operation
            .result
            .scalar_ref()
            .ok_or(ModuleError::FloatResultPolicyRequiresFloatResult(
                operation.id,
            ))?;
    let comparison =
        |kind: IeeeFloatComparisonKind, mut left: ScalarTerm, mut right: ScalarTerm| {
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::ScalarIeeeFloatComparison {
                kind,
                format,
                left,
                right,
            }
        };
    let (minimum, maximum, zero) = match format {
        IeeeFloatFormat::Binary32 => (
            IeeeFloatValue::Binary32(0xff7f_ffff),
            IeeeFloatValue::Binary32(0x7f7f_ffff),
            IeeeFloatValue::Binary32(0),
        ),
        IeeeFloatFormat::Binary64 => (
            IeeeFloatValue::Binary64(0xffef_ffff_ffff_ffff),
            IeeeFloatValue::Binary64(0x7fef_ffff_ffff_ffff),
            IeeeFloatValue::Binary64(0),
        ),
    };
    let bounds = |value| {
        vec![
            comparison(
                IeeeFloatComparisonKind::LessOrEqual,
                ScalarTerm::IeeeFloatLiteral(minimum),
                ScalarTerm::value(value, scalar_type),
            ),
            comparison(
                IeeeFloatComparisonKind::LessOrEqual,
                ScalarTerm::value(value, scalar_type),
                ScalarTerm::IeeeFloatLiteral(maximum),
            ),
        ]
    };
    let mut requirements = bounds(left);
    requirements.extend(bounds(right));
    requirements.push(comparison(
        IeeeFloatComparisonKind::Equal,
        ScalarTerm::value(raw_result, scalar_type),
        ScalarTerm::value(raw_result, scalar_type),
    ));
    if rule == terminal_psi::FloatSaturationRule::FiniteDivisionOverflow {
        requirements.push(comparison(
            IeeeFloatComparisonKind::NotEqual,
            ScalarTerm::value(right, scalar_type),
            ScalarTerm::IeeeFloatLiteral(zero),
        ));
    }
    Ok(vec![Proposition::Implication {
        premise: Box::new(Proposition::Conjunction(requirements)),
        conclusion: Box::new(Proposition::Conjunction(bounds(result.id))),
    }])
}

fn finite_bounds(
    format: IeeeFloatFormat,
    value: ValueId,
    kind: IeeeFloatComparisonKind,
) -> Vec<Proposition> {
    let (negative_infinity, positive_infinity) = match format {
        IeeeFloatFormat::Binary32 => (
            IeeeFloatValue::Binary32(0xff80_0000),
            IeeeFloatValue::Binary32(0x7f80_0000),
        ),
        IeeeFloatFormat::Binary64 => (
            IeeeFloatValue::Binary64(0xfff0_0000_0000_0000),
            IeeeFloatValue::Binary64(0x7ff0_0000_0000_0000),
        ),
    };
    let payload = ScalarTerm::value(value, ScalarType::IeeeFloat(format));
    vec![
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left: ScalarTerm::IeeeFloatLiteral(negative_infinity),
            right: payload.clone(),
        },
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left: payload,
            right: ScalarTerm::IeeeFloatLiteral(positive_infinity),
        },
    ]
}
