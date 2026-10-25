//! Division/remainder formation under the selected integer policy.
//!
//! Mathematical Int, Exact, Wrapping and Saturating require a proven nonzero divisor. Exact
//! signed operations additionally exclude `MIN / -1` and `MIN % -1`. Trapping
//! retains those conditions as runtime crash predicates. The same live
//! integer proof supplies the selected policy's formation conditions.

use super::visit_binary_operators;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_policy::{
    IntegerFormationCondition, IntegerPolicyPrimitive, integer_policy_bridge,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

use super::total_specification::{
    AbstractSpecificationBindings, abstract_specification_builtin_arithmetic,
    abstract_specification_interval, direct_abstract_operand,
};
use super::{Interval, ValueEnvironment, ordered_values};
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

fn definedness_conditions(
    operator: BinaryOperator,
    domain: ArithmeticDomain,
) -> ExactDefinednessConditions {
    let primitive = match operator {
        BinaryOperator::Divide => IntegerPolicyPrimitive::Divide,
        BinaryOperator::Modulo => IntegerPolicyPrimitive::Remainder,
        _ => unreachable!("definedness is only queried for division and remainder"),
    };
    let conditions = integer_policy_bridge(primitive, domain).formation_conditions;
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

/// Report each missing formation condition. Returns whether the operation
/// owed at least one condition and every one was proven: a decision lowering
/// may consume. A provably zero divisor rejected elsewhere decides nothing.
fn report_partial(
    operator: BinaryOperator,
    domain: ArithmeticDomain,
    proven: ProvenDefinedness,
    owner: &str,
    provably_zero_is_prevalidated: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let conditions = definedness_conditions(operator, domain);
    let missing_nonzero = conditions.nonzero_divisor
        && !(proven.nonzero_divisor
            || provably_zero_is_prevalidated && proven.provably_zero_divisor);
    let signed_overflow_pair =
        conditions.signed_result_representable && !proven.signed_pair_excluded;
    if !missing_nonzero && !signed_overflow_pair {
        return (conditions.nonzero_divisor || conditions.signed_result_representable)
            && (!conditions.nonzero_divisor || proven.nonzero_divisor);
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
        "partial {} {operation} in {owner}: {}; discharge every definedness condition with independently accepted prior facts",
        domain.name().to_ascii_lowercase(),
        missing.join(" and "),
    )));
    false
}

/// Operands have already been checked under their selected primitive meaning.
/// Intervals and live unequal/order relations discharge the same conditions in
/// executable bodies and specification facts. A remembered source guard is not
/// evidence after its operand has been written or lost at a control-flow join.
/// Returns whether this occurrence's formation conditions are decided.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_operands(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    binary: &TableBinaryExpression,
    domain: ArithmeticDomain,
    primitive: PrimitiveType,
    left: Interval,
    right: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !primitive.is_fixed_integer()
        || !matches!(
            binary.operator,
            BinaryOperator::Divide | BinaryOperator::Modulo
        )
    {
        return false;
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
    report_partial(binary.operator, domain, proven, owner, true, diagnostics)
}

/// Declaration operands share the policy's formation conditions without
/// claiming executable-prefix relations or a caller frame.
pub(super) fn validate_interval_operands(
    operator: BinaryOperator,
    domain: ArithmeticDomain,
    primitive: PrimitiveType,
    left: Interval,
    right: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo) {
        return false;
    }
    report_partial(
        operator,
        domain,
        ProvenDefinedness::from_intervals(primitive, Some(left), Some(right)),
        owner,
        false,
        diagnostics,
    )
}

pub(super) fn validate_abstract(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    owner: &str,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    visit_binary_operators(
        program,
        expression,
        &[BinaryOperator::Divide, BinaryOperator::Modulo],
        |expression, binary| {
            if !abstract_specification_builtin_arithmetic(program, bindings, expression) {
                return;
            }
            let Some((primitive, domain)) = direct_abstract_operand(program, bindings, expression)
            else {
                return;
            };
            if domain != ArithmeticDomain::Exact || !primitive.is_fixed_integer() {
                return;
            }
            _ = report_partial(
                binary.operator,
                ArithmeticDomain::Exact,
                ProvenDefinedness::from_intervals(
                    primitive,
                    abstract_specification_interval(program, bindings, environment, binary.left),
                    abstract_specification_interval(program, bindings, environment, binary.right),
                ),
                owner,
                false,
                diagnostics,
            );
        },
    );
}

/// Mathematical integers have the same nonzero obligation but no finite
/// signed overflow pair. The containing fact has not yet entered hypotheses.
pub(in crate::proof_contracts) fn validate_proof_integer(
    program: &dyn crate::CheckingInput,
    owner_symbol: symbols::SymbolHandle,
    binary: &TableBinaryExpression,
    hypotheses: &[ExpressionHandle],
    selections: &[super::DispatchSelection],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    _ = report_partial(
        binary.operator,
        ArithmeticDomain::Exact,
        ProvenDefinedness {
            nonzero_divisor: crate::proof_contracts::contract_entailment::proof_integer_nonzero(
                program,
                owner_symbol,
                binary.right,
                hypotheses,
                selections,
            ),
            provably_zero_divisor: false,
            signed_pair_excluded: true,
        },
        owner,
        false,
        diagnostics,
    );
}
