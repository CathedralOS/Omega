//! Discharged-goal elision judgment for proof-bearing scalar leaves.
//!
//! A proof-bearing scalar leaf whose canonical goal is decided by literal
//! bindings alone — never by a producer-chosen sufficient form — may rewrite
//! in place to the goal-free leaf that denotes the same result. The judgment
//! substitutes only the literals the caller has already established and
//! re-projects the canonical kernel proposition; a goal that stays symbolic
//! keeps its obligation row and its evidence requirement.
//!
//! The replacement keeps the operation's identity, result declaration, fuel,
//! and frontier policy. Its result equation replaces the discharged leaf's in
//! every reconstructed axiom snapshot: that transport belongs to the rewrite's
//! independent check, not to this judgment.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition, ScalarTerm, ScalarType,
    ValueId,
};
use terminal_psi::{Operation, OperationKind};

use super::{
    CanonicalScalarGoal, ProofBearingScalarLeafSemantics, proof_bearing_scalar_leaf_semantics,
};
use crate::{OperationSemanticError, ScalarLeafLiteral};

/// The exact rewrite one discharged proof-bearing scalar leaf admits.
///
/// `replacement` is the goal-free leaf computing the same result; when it is
/// itself a literal producer, `result_literal` carries that literal so a
/// folding driver can seed it into its own environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofBearingScalarLeafElision {
    semantics: ProofBearingScalarLeafSemantics,
    replacement: OperationKind,
    result_literal: Option<IntegerValue>,
}

impl ProofBearingScalarLeafElision {
    /// The discharged leaf's reconstructed semantics: canonical goal,
    /// denotation, and result equation.
    pub const fn semantics(&self) -> &ProofBearingScalarLeafSemantics {
        &self.semantics
    }

    /// The obligation identity this rewrite consumes.
    pub const fn obligation(&self) -> ObligationId {
        self.semantics.obligation()
    }

    /// The goal-free operation kind the leaf rewrites to.
    pub const fn replacement(&self) -> &OperationKind {
        &self.replacement
    }

    /// The literal the rewritten leaf denotes when it is a literal producer.
    pub const fn result_literal(&self) -> Option<IntegerValue> {
        self.result_literal
    }
}

/// Decide whether `operation` is a proof-bearing scalar leaf whose canonical
/// goal holds without proof evidence under `literals`, and if so choose its
/// exact goal-free replacement.
///
/// `Ok(None)` covers every leaf the rule cannot rewrite: a non-scalar row, a
/// symbolic goal, and a discharged goal with no goal-free form (a checked
/// divide or a covered same-type cast keeps its obligation). Errors mean the
/// row does not match its declared semantic schema.
pub fn elidable_proof_bearing_scalar_leaf(
    operation: &Operation,
    literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Option<ProofBearingScalarLeafElision>, OperationSemanticError> {
    let Some(semantics) = proof_bearing_scalar_leaf_semantics(operation, value_types)? else {
        return Ok(None);
    };
    let ScalarType::Integer(result_type) = operation
        .result
        .scalar_ref()
        .expect("proof-bearing semantics requires a scalar result")
        .scalar_type
    else {
        unreachable!("proof-bearing scalar results are declared integers")
    };
    if !canonical_goal_is_discharged(semantics.canonical_goal(), literals) {
        return Ok(None);
    }
    if let Some(value) = discharged_literal(operation, &semantics, literals, result_type) {
        return Ok(Some(ProofBearingScalarLeafElision {
            semantics,
            replacement: OperationKind::IntegerConstant { value },
            result_literal: Some(value),
        }));
    }
    let replacement = match &operation.kind {
        OperationKind::ExactIntegerShiftLeft { value, count, .. } => {
            OperationKind::WrappingIntegerShiftLeft {
                value: *value,
                count: *count,
            }
        }
        OperationKind::ExactIntegerShiftRight { value, count, .. } => {
            OperationKind::WrappingIntegerShiftRight {
                value: *value,
                count: *count,
            }
        }
        OperationKind::ExactIntegerAdd { left, right, .. } => OperationKind::WrappingIntegerAdd {
            left: *left,
            right: *right,
        },
        OperationKind::ExactIntegerSubtract { left, right, .. } => {
            OperationKind::WrappingIntegerSubtract {
                left: *left,
                right: *right,
            }
        }
        OperationKind::ExactIntegerMultiply { left, right, .. } => {
            OperationKind::WrappingIntegerMultiply {
                left: *left,
                right: *right,
            }
        }
        OperationKind::IntegerExactCast { operand, .. } => {
            let Some(ScalarType::Integer(source_type)) = value_types.get(operand).copied() else {
                return Ok(None);
            };
            if !source_type.can_widen_to(result_type) {
                return Ok(None);
            }
            OperationKind::IntegerWiden { operand: *operand }
        }
        _ => return Ok(None),
    };
    Ok(Some(ProofBearingScalarLeafElision {
        semantics,
        replacement,
        result_literal: None,
    }))
}

/// Decide one canonical goal under `literals` alone.
///
/// Operand terms carry value identities; substitution replaces each known
/// literal before the canonical kernel proposition is re-projected. The
/// divide goals never project to `Truth`, so their literal divisor decides
/// the same disjunction the kernel emits: nonzero, and for a signed `-1`
/// divisor a literal dividend above the minimum.
fn canonical_goal_is_discharged(
    goal: &CanonicalScalarGoal,
    literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
) -> bool {
    let goal = literal_substituted_goal(goal, literals);
    match &goal {
        CanonicalScalarGoal::NonzeroDivisor { divisor, .. } => {
            matches!(divisor.integer_value(), Some((_, value)) if !is_zero(value))
        }
        CanonicalScalarGoal::ExactDivisionDefined {
            integer_type,
            left,
            right,
        } => match right.integer_value() {
            Some((_, divisor)) if !is_zero(divisor) => {
                integer_type.sign() == IntegerSign::Unsigned
                    || divisor != IntegerValue::Signed(-1)
                    || matches!(
                        left.integer_value(),
                        Some((_, dividend)) if dividend != integer_type.minimum_value()
                    )
            }
            _ => false,
        },
        _ => goal
            .kernel_proposition()
            .is_ok_and(|proposition| proposition == Proposition::Truth),
    }
}

/// The literal one discharged leaf denotes, or `None` when the result still
/// depends on an unbound operand.
///
/// Denotation evaluation covers every all-literal row. The identity rows
/// cover results fixed without binding every operand: a self-subtraction, a
/// zero multiplicand, a zero shifted value, and a `±1` divisor remainder are
/// zero for every admitted operand — the discharged goal has already ruled
/// out the cases where the operation could fail.
fn discharged_literal(
    operation: &Operation,
    semantics: &ProofBearingScalarLeafSemantics,
    literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
    result_type: IntegerType,
) -> Option<IntegerValue> {
    if let Some((denoted_type, value)) =
        literal_scalar_term(semantics.denotation(), literals).integer_value()
        && denoted_type == result_type
    {
        return Some(value);
    }
    let zero = || match result_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    };
    let literal_of = |value: &ValueId| match literals.get(value) {
        Some(ScalarLeafLiteral::Integer(literal)) => Some(*literal),
        _ => None,
    };
    match &operation.kind {
        OperationKind::ExactIntegerSubtract { left, right, .. } if left == right => Some(zero()),
        OperationKind::ExactIntegerMultiply { left, right, .. }
            if is_zero_opt(literal_of(left)) || is_zero_opt(literal_of(right)) =>
        {
            Some(zero())
        }
        OperationKind::ExactIntegerShiftLeft { value, .. }
        | OperationKind::ExactIntegerShiftRight { value, .. }
            if is_zero_opt(literal_of(value)) =>
        {
            Some(zero())
        }
        OperationKind::ExactIntegerRemainder { right, .. } if is_one(literal_of(right)) => {
            Some(zero())
        }
        OperationKind::WrappingIntegerRemainder { right, .. }
        | OperationKind::SaturatingIntegerRemainder { right, .. }
            if is_one(literal_of(right))
                || (result_type.sign() == IntegerSign::Signed
                    && literal_of(right) == Some(IntegerValue::Signed(-1))) =>
        {
            Some(zero())
        }
        _ => None,
    }
}

/// Rebuild a canonical goal with every literally bound operand substituted.
fn literal_substituted_goal(
    goal: &CanonicalScalarGoal,
    literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
) -> CanonicalScalarGoal {
    let substitute = |term: &ScalarTerm| literal_scalar_term(term, literals);
    match goal {
        CanonicalScalarGoal::ExactCastRepresentable {
            source_type,
            target_type,
            operand,
        } => CanonicalScalarGoal::ExactCastRepresentable {
            source_type: *source_type,
            target_type: *target_type,
            operand: substitute(operand),
        },
        CanonicalScalarGoal::ExactShiftCount {
            value_type,
            count_type,
            count,
        } => CanonicalScalarGoal::ExactShiftCount {
            value_type: *value_type,
            count_type: *count_type,
            count: substitute(count),
        },
        CanonicalScalarGoal::ExactShiftLeftRepresentable {
            value_type,
            count_type,
            value,
            count,
        } => CanonicalScalarGoal::ExactShiftLeftRepresentable {
            value_type: *value_type,
            count_type: *count_type,
            value: substitute(value),
            count: substitute(count),
        },
        CanonicalScalarGoal::ExactArithmeticRepresentable {
            integer_type,
            expression,
        } => CanonicalScalarGoal::ExactArithmeticRepresentable {
            integer_type: *integer_type,
            expression: literal_scalar_term(expression, literals),
        },
        CanonicalScalarGoal::ExactDivisionDefined {
            integer_type,
            left,
            right,
        } => CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: *integer_type,
            left: substitute(left),
            right: substitute(right),
        },
        CanonicalScalarGoal::NonzeroDivisor {
            integer_type,
            divisor,
        } => CanonicalScalarGoal::NonzeroDivisor {
            integer_type: *integer_type,
            divisor: substitute(divisor),
        },
    }
}

/// Rebuild a scalar term with every literally bound leaf substituted.
///
/// Goals and denotations carry value terms and one level of exact-arithmetic
/// composition; other composite shapes pass through unchanged.
fn literal_scalar_term(
    term: &ScalarTerm,
    literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
) -> ScalarTerm {
    if let ScalarTerm::Value {
        id,
        scalar_type: ScalarType::Integer(integer_type),
    } = term
        && let Some(ScalarLeafLiteral::Integer(value)) = literals.get(id)
        && let Ok(literal) = ScalarTerm::integer(*integer_type, *value)
    {
        return literal;
    }
    let rebuild = |left: &ScalarTerm, right: &ScalarTerm| {
        (
            literal_scalar_term(left, literals),
            literal_scalar_term(right, literals),
        )
    };
    match term {
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => ScalarTerm::integer_exact_cast(
            *source_type,
            *target_type,
            literal_scalar_term(operand, literals),
        )
        .unwrap_or_else(|_| term.clone()),
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::exact_integer_shift_left(
            *value_type,
            *count_type,
            literal_scalar_term(value, literals),
            literal_scalar_term(count, literals),
        )
        .unwrap_or_else(|_| term.clone()),
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::exact_integer_shift_right(
            *value_type,
            *count_type,
            literal_scalar_term(value, literals),
            literal_scalar_term(count, literals),
        )
        .unwrap_or_else(|_| term.clone()),
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::exact_integer_add(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::exact_integer_subtract(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::exact_integer_multiply(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::exact_integer_divide(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::exact_integer_remainder(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::wrapping_integer_divide(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::wrapping_integer_remainder(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::saturating_integer_divide(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = rebuild(left, right);
            ScalarTerm::saturating_integer_remainder(*scalar_type, left, right)
                .unwrap_or_else(|_| term.clone())
        }
        _ => term.clone(),
    }
}

fn is_zero(value: IntegerValue) -> bool {
    matches!(value, IntegerValue::Signed(0) | IntegerValue::Unsigned(0))
}

fn is_zero_opt(value: Option<IntegerValue>) -> bool {
    value.is_some_and(is_zero)
}

fn is_one(value: Option<IntegerValue>) -> bool {
    matches!(
        value,
        Some(IntegerValue::Signed(1) | IntegerValue::Unsigned(1))
    )
}
