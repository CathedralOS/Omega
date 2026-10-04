//! Exact division/remainder formation for runtime and specification expressions.
//!
//! Both operations require a nonzero divisor. Signed carriers additionally
//! exclude the one unrepresentable primitive pair, `MIN / -1` (and the same
//! hardware-definedness pair for remainder). The fact containing the operation
//! is checked before it can enter the value environment.

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_policy::{
    IntegerFormationCondition, IntegerPolicyPrimitive, integer_policy_bridge,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

use super::total_specification::{
    AbstractSpecificationBindings, abstract_specification_interval,
    abstract_specification_place_type,
};
use super::{Interval, ValueEnvironment, ordered_values};
use crate::proof_contracts::arithmetic_domains::expression_analysis::analyze;
use typed_trees::expression::TableBinaryExpression;

fn signed_minimum(primitive: PrimitiveType) -> Option<i64> {
    match primitive {
        PrimitiveType::I8 => Some(i8::MIN as i64),
        PrimitiveType::I16 => Some(i16::MIN as i64),
        PrimitiveType::I32 => Some(i32::MIN as i64),
        PrimitiveType::I64 => Some(i64::MIN),
        _ => None,
    }
}

fn fixed_integer(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::U8
            | PrimitiveType::I16
            | PrimitiveType::U16
            | PrimitiveType::I32
            | PrimitiveType::U32
            | PrimitiveType::I64
            | PrimitiveType::U64
    )
}

fn may_contain(interval: Option<Interval>, value: i64) -> bool {
    let Some(interval) = interval else {
        return true;
    };
    interval.low().is_none_or(|low| low <= value)
        && interval.high().is_none_or(|high| value <= high)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExactDefinednessConditions {
    nonzero_divisor: bool,
    signed_result_representable: bool,
}

fn exact_definedness_conditions(operator: BinaryOperator) -> ExactDefinednessConditions {
    let primitive = match operator {
        BinaryOperator::Divide => IntegerPolicyPrimitive::Divide,
        BinaryOperator::Modulo => IntegerPolicyPrimitive::Remainder,
        _ => unreachable!("exact definedness is only queried for division and remainder"),
    };
    let conditions = integer_policy_bridge(primitive, ArithmeticDomain::Exact).formation_conditions;
    ExactDefinednessConditions {
        nonzero_divisor: conditions.contains(&IntegerFormationCondition::NonZeroDivisor),
        signed_result_representable: conditions
            .contains(&IntegerFormationCondition::ResultRepresentable),
    }
}

struct ProvenDefinedness {
    nonzero_divisor: bool,
    provably_zero_divisor: bool,
    signed_pair_excluded: bool,
}

impl ProvenDefinedness {
    fn from_intervals(
        primitive: PrimitiveType,
        left: Option<Interval>,
        right: Option<Interval>,
    ) -> Self {
        Self {
            nonzero_divisor: right.is_some_and(Interval::excludes_zero),
            provably_zero_divisor: right.is_some_and(Interval::is_exactly_zero),
            signed_pair_excluded: !signed_minimum(primitive)
                .is_some_and(|minimum| may_contain(left, minimum) && may_contain(right, -1)),
        }
    }
}

fn report_partial(
    operator: BinaryOperator,
    proven: ProvenDefinedness,
    owner: &str,
    provably_zero_is_prevalidated: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let conditions = exact_definedness_conditions(operator);
    let missing_nonzero = conditions.nonzero_divisor
        && !(proven.nonzero_divisor
            || provably_zero_is_prevalidated && proven.provably_zero_divisor);
    let signed_overflow_pair =
        conditions.signed_result_representable && !proven.signed_pair_excluded;
    if !missing_nonzero && !signed_overflow_pair {
        return;
    }

    let operation = if operator == BinaryOperator::Divide {
        "division"
    } else {
        "remainder"
    };
    let mut missing = Vec::new();
    if missing_nonzero {
        missing.push("the divisor must be proven nonzero");
    }
    if signed_overflow_pair {
        missing.push("the signed `MIN / -1` primitive pair must be excluded");
    }
    diagnostics.push(Diagnostic::error(format!(
        "partial exact {operation} in {owner}: {}; discharge every definedness condition with independently accepted prior facts",
        missing.join(" and "),
    )));
}

/// Operands have already been checked under their selected primitive meaning.
/// Intervals and live unequal/order relations discharge the same conditions in
/// executable bodies and specification facts. A remembered source guard is not
/// evidence after its operand has been written or lost at a control-flow join.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_operands(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    binary: &TableBinaryExpression,
    primitive: PrimitiveType,
    left: Interval,
    right: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !fixed_integer(primitive)
        || !matches!(
            binary.operator,
            BinaryOperator::Divide | BinaryOperator::Modulo
        )
    {
        return;
    }
    let excludes = |expression, value| {
        ordered_values::excludes_integer_value(
            program,
            machine,
            state,
            environment,
            expression,
            value,
        ) || super::declared_place_type_raw(program, machine, state, expression).is_some_and(
            |reference| super::domain_exclusions::type_excludes_integer(program, reference, value),
        )
    };
    let mut proven = ProvenDefinedness::from_intervals(primitive, Some(left), Some(right));
    proven.nonzero_divisor = proven.nonzero_divisor
        || excludes(binary.right, 0)
        || super::live_integer_constraints::excludes_values(
            program,
            machine,
            state,
            environment,
            &[(binary.right, 0)],
        );
    proven.signed_pair_excluded = proven.signed_pair_excluded
        || signed_minimum(primitive).is_some_and(|minimum| {
            excludes(binary.left, minimum)
                || excludes(binary.right, -1)
                || super::live_integer_constraints::excludes_values(
                    program,
                    machine,
                    state,
                    environment,
                    &[(binary.left, minimum), (binary.right, -1)],
                )
        });
    report_partial(binary.operator, proven, owner, true, diagnostics);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_concrete(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn walk(
        program: &TypedTrees,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        environment: &ValueEnvironment,
        owner: &str,
        diagnostics: &mut Vec<Diagnostic>,
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);
        let recurse = |child, diagnostics: &mut Vec<Diagnostic>, visited: &mut Vec<_>| {
            walk(
                program,
                machine,
                state,
                child,
                environment,
                owner,
                diagnostics,
                visited,
            );
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for child in crate::expression_traversal::match_children(program, *dispatch) {
                    recurse(child, diagnostics, visited);
                }
            }
            ExpressionNode::Binary(binary) => {
                recurse(binary.left, diagnostics, visited);
                recurse(binary.right, diagnostics, visited);
                if !matches!(
                    binary.operator,
                    BinaryOperator::Divide | BinaryOperator::Modulo
                ) || !crate::has_builtin_bound_expression_meaning(
                    program, machine, state, expression,
                ) {
                    return;
                }
                let mut ignored = Vec::new();
                let operation = analyze(
                    program,
                    machine,
                    state,
                    expression,
                    environment,
                    None,
                    ArithmeticDomain::Exact,
                    owner,
                    &mut ignored,
                );
                if operation.domain.unwrap_or(ArithmeticDomain::Exact) != ArithmeticDomain::Exact {
                    return;
                }
                let Some(primitive) = operation
                    .primitive
                    .filter(|primitive| fixed_integer(*primitive))
                else {
                    return;
                };
                let mut ignored = Vec::new();
                let left = analyze(
                    program,
                    machine,
                    state,
                    binary.left,
                    environment,
                    None,
                    ArithmeticDomain::Exact,
                    owner,
                    &mut ignored,
                )
                .interval;
                let mut ignored = Vec::new();
                let right = analyze(
                    program,
                    machine,
                    state,
                    binary.right,
                    environment,
                    None,
                    ArithmeticDomain::Exact,
                    owner,
                    &mut ignored,
                )
                .interval;
                validate_operands(
                    program,
                    machine,
                    state,
                    environment,
                    binary,
                    primitive,
                    left,
                    right,
                    owner,
                    diagnostics,
                );
            }
            ExpressionNode::ArrayLiteral(values) => {
                for child in program.expression_table.expression_handles(*values) {
                    recurse(*child, diagnostics, visited);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                recurse(atomic.value, diagnostics, visited);
                recurse(atomic.result, diagnostics, visited);
            }
            ExpressionNode::Cast(cast) => recurse(cast.value, diagnostics, visited),
            ExpressionNode::Call(call) => {
                recurse(call.receiver, diagnostics, visited);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    recurse(*argument, diagnostics, visited);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                recurse(indexed.collection, diagnostics, visited);
                recurse(indexed.index, diagnostics, visited);
            }
            ExpressionNode::Member(member) => recurse(member.receiver, diagnostics, visited),
            ExpressionNode::Borrow(value) => recurse(value.target, diagnostics, visited),
            ExpressionNode::Unary(unary) => recurse(unary.operand, diagnostics, visited),
            ExpressionNode::Range(range) => {
                recurse(range.start, diagnostics, visited);
                recurse(range.end, diagnostics, visited);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    recurse(field.value, diagnostics, visited);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    walk(
        program,
        machine,
        state,
        expression,
        environment,
        owner,
        diagnostics,
        &mut Vec::new(),
    );
}

fn direct_abstract_operand(
    program: &TypedTrees,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => Some((
            program.primitive_type_reference(cast.target_type)?,
            cast.domain,
        )),
        ExpressionNode::Borrow(value) => direct_abstract_operand(program, bindings, value.target),
        _ => {
            let type_reference = abstract_specification_place_type(program, bindings, expression)?;
            Some((
                program.primitive_type_reference(type_reference)?,
                program.arithmetic_domain_for_type_reference(type_reference),
            ))
        }
    }
}

pub(super) fn validate_abstract(
    program: &TypedTrees,
    expression: ExpressionHandle,
    owner: &str,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn walk(
        program: &TypedTrees,
        expression: ExpressionHandle,
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        environment: &ValueEnvironment,
        diagnostics: &mut Vec<Diagnostic>,
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);
        let recurse = |child, diagnostics: &mut Vec<Diagnostic>, visited: &mut Vec<_>| {
            walk(
                program,
                child,
                owner,
                bindings,
                environment,
                diagnostics,
                visited,
            );
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for child in crate::expression_traversal::match_children(program, *dispatch) {
                    recurse(child, diagnostics, visited);
                }
            }
            ExpressionNode::Binary(binary) => {
                recurse(binary.left, diagnostics, visited);
                recurse(binary.right, diagnostics, visited);
                if !matches!(
                    binary.operator,
                    BinaryOperator::Divide | BinaryOperator::Modulo
                ) {
                    return;
                }
                let left_operand = direct_abstract_operand(program, bindings, binary.left);
                let right_operand = direct_abstract_operand(program, bindings, binary.right);
                let selected = match (left_operand, right_operand) {
                    (Some((_, ArithmeticDomain::Exact)), Some((_, right))) => Some(right),
                    (Some((_, left)), Some(_)) => Some(left),
                    (Some((_, domain)), None) | (None, Some((_, domain))) => Some(domain),
                    (None, None) => None,
                };
                if selected.unwrap_or(ArithmeticDomain::Exact) != ArithmeticDomain::Exact {
                    return;
                }
                let Some(primitive) = left_operand
                    .or(right_operand)
                    .map(|(primitive, _)| primitive)
                    .filter(|primitive| fixed_integer(*primitive))
                else {
                    return;
                };
                report_partial(
                    binary.operator,
                    ProvenDefinedness::from_intervals(
                        primitive,
                        abstract_specification_interval(
                            program,
                            bindings,
                            environment,
                            binary.left,
                        ),
                        abstract_specification_interval(
                            program,
                            bindings,
                            environment,
                            binary.right,
                        ),
                    ),
                    owner,
                    false,
                    diagnostics,
                );
            }
            ExpressionNode::ArrayLiteral(values) => {
                for child in program.expression_table.expression_handles(*values) {
                    recurse(*child, diagnostics, visited);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                recurse(atomic.value, diagnostics, visited);
                recurse(atomic.result, diagnostics, visited);
            }
            ExpressionNode::Cast(cast) => recurse(cast.value, diagnostics, visited),
            ExpressionNode::Call(call) => {
                recurse(call.receiver, diagnostics, visited);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    recurse(*argument, diagnostics, visited);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                recurse(indexed.collection, diagnostics, visited);
                recurse(indexed.index, diagnostics, visited);
            }
            ExpressionNode::Member(member) => recurse(member.receiver, diagnostics, visited),
            ExpressionNode::Borrow(value) => recurse(value.target, diagnostics, visited),
            ExpressionNode::Unary(unary) => recurse(unary.operand, diagnostics, visited),
            ExpressionNode::Range(range) => {
                recurse(range.start, diagnostics, visited);
                recurse(range.end, diagnostics, visited);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    recurse(field.value, diagnostics, visited);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    walk(
        program,
        expression,
        owner,
        bindings,
        environment,
        diagnostics,
        &mut Vec::new(),
    );
}

#[cfg(test)]
mod tests;
