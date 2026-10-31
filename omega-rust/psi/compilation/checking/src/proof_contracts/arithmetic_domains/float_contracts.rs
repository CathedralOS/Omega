//! Ordinary IEEE contracts describe normal results, not result-type identity.
//! Projection reads the selected declaration and its reserved result binding;
//! it never evaluates a callee body or rereads mutable actuals. Exit checking
//! independently proves each guarantee from the actual returned occurrence.
//! Both consume the same landed source enclosure and selected comparison
//! meaning. An unknown enclosure supplies no proof, and a receiving predicate
//! never supplies evidence about the input of an Exact conversion.

use super::float_arithmetic::{
    FloatSourceBounds, float_bound_from, float_literal_value, float_source_bounds,
};
use super::value_environment::FloatInterval;
use crate::machine_calls::calls::CalleeContract;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Publish a selected call's normal IEEE bounds on its completed occurrence.
/// The source declaration owes these guarantees at every exit; no receiving
/// annotation or private callee-body inference contributes a predicate here.
pub fn normal_return_float_predicates(
    program: &dyn crate::CheckingInput,
    caller: symbols::SymbolHandle,
    expression: ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    predicates: &mut Vec<facts::FloatPredicate>,
) {
    if !call.selects_only_nominal_route() {
        return;
    }
    let Some(callee) =
        crate::machine_calls::calls::callee_entry_contract(program, call.target_symbol)
    else {
        return;
    };
    let Some(interval) = normal_return_interval(program, caller, expression, call) else {
        return;
    };
    let format = match program.primitive_type_reference(callee.return_type) {
        Some(PrimitiveType::F32) => numerics::literals::FloatFormat::F32,
        Some(PrimitiveType::F64) => numerics::literals::FloatFormat::F64,
        _ => return,
    };
    if let (Some(minimum), Some(maximum)) = (interval.low, interval.high)
        && minimum == maximum
        && let Some(predicate) = facts::FloatPredicate::comparison(
            format,
            BinaryOperator::Equal,
            &numerics::bignum::ExactFloat::from_f64(minimum),
        )
    {
        predicates.push(predicate);
        return;
    }
    for (operator, bound) in [
        (BinaryOperator::GreaterOrEqual, interval.low),
        (BinaryOperator::LessOrEqual, interval.high),
    ] {
        if let Some(predicate) = bound.and_then(|bound| {
            facts::FloatPredicate::comparison(
                format,
                operator,
                &numerics::bignum::ExactFloat::from_f64(bound),
            )
        }) {
            predicates.push(predicate);
        }
    }
}

pub(super) fn normal_return_interval(
    program: &dyn crate::CheckingInput,
    caller: symbols::SymbolHandle,
    expression: ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<FloatInterval> {
    if !call.selects_only_nominal_route()
        || !crate::machine_calls::contract_dependencies::independent_call_application(
            program,
            caller,
            call.target_symbol,
            typed_trees::references::StaticArgumentCall::Expression(expression, call),
        )
    {
        return None;
    }
    let callee = crate::machine_calls::calls::callee_entry_contract(program, call.target_symbol)?;
    let primitive = program.primitive_type_reference(callee.return_type)?;
    if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
        return None;
    }
    if let Some(machine) = program.machine_by_symbol(callee.callable)
        && machine.target_family_interface
    {
        // A portable caller receives only bounds shared by every checked
        // target body. Picking the host or one representative would expose a
        // private target's stronger guarantee as the family's public contract.
        let mut possible: Option<FloatInterval> = None;
        for member in crate::machine_calls::calls::target_family_members(program, machine)? {
            let body = program.machine(member.machine)?;
            let entry = program.machine_states(body).first()?;
            let target = crate::machine_calls::calls::callee_entry_contract(program, entry.symbol)?;
            if program.primitive_type_reference(target.return_type) != Some(primitive) {
                return None;
            }
            let interval = declared_result_interval(program, &target, primitive);
            possible = Some(possible.map_or(interval, |previous| previous.union(interval)));
        }
        let interval = possible?;
        return (interval != FloatInterval::UNBOUNDED).then_some(interval);
    }
    let interval = declared_result_interval(program, &callee, primitive);
    (interval != FloatInterval::UNBOUNDED).then_some(interval)
}

fn declared_result_interval(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    primitive: PrimitiveType,
) -> FloatInterval {
    program
        .contract_expressions(callee.contracts.iter(), SignatureContractKind::Ensures)
        .fold(FloatInterval::UNBOUNDED, |interval, expression| {
            interval.intersect(project(program, callee, expression, primitive))
        })
}

fn project(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    expression: ExpressionHandle,
    primitive: PrimitiveType,
) -> FloatInterval {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        if let Some(predicates) =
            crate::mathematical_applications::float_predicates(program, expression)
        {
            return predicates.into_iter().fold(
                FloatInterval::UNBOUNDED,
                |interval, projection| {
                    if crate::reserved_result_owner(program, projection.subject).is_some_and(
                        |(owner, reference)| {
                            owner == callee.callable && reference == callee.return_type
                        },
                    ) && matches!(
                        (primitive, projection.predicate.format()),
                        (PrimitiveType::F32, numerics::literals::FloatFormat::F32)
                            | (PrimitiveType::F64, numerics::literals::FloatFormat::F64)
                    ) {
                        interval.intersect(
                            float_bound_from(
                                projection.operator,
                                projection.bound.to_f64(),
                                true,
                                primitive,
                            )
                            .unwrap_or(FloatInterval::UNBOUNDED),
                        )
                    } else {
                        interval
                    }
                },
            );
        }
        return FloatInterval::UNBOUNDED;
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        let left = project(program, callee, binary.left, primitive);
        let right = project(program, callee, binary.right, primitive);
        return if binary.operator == BinaryOperator::And {
            left.intersect(right)
        } else {
            left.union(right)
        };
    }
    let result = |expression| {
        crate::reserved_result_owner(program, expression).is_some_and(|(owner, reference)| {
            owner == callee.callable && reference == callee.return_type
        })
    };
    let (other, result_on_left) = if result(binary.left) {
        (binary.right, true)
    } else if result(binary.right) {
        (binary.left, false)
    } else {
        return FloatInterval::UNBOUNDED;
    };
    let other_type = crate::parameter_expression_result_type_reference(
        program,
        callee.callable,
        callee.parameters,
        other,
    );
    let operand_types = if result_on_left {
        [Some(callee.return_type), other_type]
    } else {
        [other_type, Some(callee.return_type)]
    };
    if !comparison_meaning(
        program,
        callee.callable,
        expression,
        binary.operator,
        &operand_types,
    ) {
        return FloatInterval::UNBOUNDED;
    }
    if let Some(literal) = float_literal_value(program, other)
        && let Some(interval) =
            float_bound_from(binary.operator, literal, result_on_left, primitive)
    {
        return interval;
    }
    let Some(parameter_interval) =
        callee_parameter_interval(program, callee, other, other_type, primitive)
    else {
        return FloatInterval::UNBOUNDED;
    };
    let operator = if result_on_left {
        binary.operator
    } else {
        binary.operator.mirrored()
    };
    match operator {
        BinaryOperator::Equal => parameter_interval,
        BinaryOperator::GreaterOrEqual => FloatInterval {
            low: parameter_interval.low,
            high: None,
        },
        BinaryOperator::Greater => FloatInterval {
            low: parameter_interval.low.and_then(|low| {
                float_bound_from(BinaryOperator::Greater, low, true, primitive)?.low
            }),
            high: None,
        },
        BinaryOperator::LessOrEqual => FloatInterval {
            low: None,
            high: parameter_interval.high,
        },
        BinaryOperator::Less => FloatInterval {
            low: None,
            high: parameter_interval.high.and_then(|high| {
                float_bound_from(BinaryOperator::Less, high, true, primitive)?.high
            }),
        },
        _ => FloatInterval::UNBOUNDED,
    }
}

fn callee_parameter_interval(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    other: ExpressionHandle,
    other_type: Option<TypeReferenceHandle>,
    primitive: PrimitiveType,
) -> Option<FloatInterval> {
    let path = super::place_path(program, other)?;
    let parameter = callee
        .parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == path)?;
    let declared = other_type
        .or(Some(parameter.type_reference))
        .and_then(|reference| {
            super::float_arithmetic::float_range_constraint_interval(program, reference)
        })
        .unwrap_or(FloatInterval::UNBOUNDED);
    let interval = program
        .contract_expressions(callee.contracts.iter(), SignatureContractKind::Requires)
        .fold(declared, |interval, expression| {
            interval.intersect(project_parameter(
                program,
                callee,
                expression,
                &path,
                parameter.type_reference,
                primitive,
            ))
        });
    (interval != FloatInterval::UNBOUNDED).then_some(interval)
}

fn project_parameter(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    expression: ExpressionHandle,
    parameter_name: &str,
    parameter_type: TypeReferenceHandle,
    primitive: PrimitiveType,
) -> FloatInterval {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return FloatInterval::UNBOUNDED;
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        let left = project_parameter(
            program,
            callee,
            binary.left,
            parameter_name,
            parameter_type,
            primitive,
        );
        let right = project_parameter(
            program,
            callee,
            binary.right,
            parameter_name,
            parameter_type,
            primitive,
        );
        return if binary.operator == BinaryOperator::And {
            left.intersect(right)
        } else {
            left.union(right)
        };
    }
    let matches_parameter =
        |operand| super::place_path(program, operand).as_deref() == Some(parameter_name);
    let (other, parameter_on_left) = if matches_parameter(binary.left) {
        (binary.right, true)
    } else if matches_parameter(binary.right) {
        (binary.left, false)
    } else {
        return FloatInterval::UNBOUNDED;
    };
    let other_type = crate::parameter_expression_result_type_reference(
        program,
        callee.callable,
        callee.parameters,
        other,
    );
    let operand_types = if parameter_on_left {
        [Some(parameter_type), other_type]
    } else {
        [other_type, Some(parameter_type)]
    };
    if !comparison_meaning(
        program,
        callee.callable,
        expression,
        binary.operator,
        &operand_types,
    ) {
        return FloatInterval::UNBOUNDED;
    }
    float_literal_value(program, other)
        .and_then(|literal| {
            float_bound_from(binary.operator, literal, parameter_on_left, primitive)
        })
        .unwrap_or(FloatInterval::UNBOUNDED)
}

fn comparison_meaning(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    crate::has_builtin_or_sealed_float_comparison_meaning(
        program,
        owner,
        expression,
        operator,
        operand_types,
    )
}

/// Prove an IEEE postcondition using the returned value at its own observation.
/// Only the exact owner's reserved result is substituted; parameter names in
/// another state cannot become the current state's same-spelled values.
pub fn float_return_postcondition(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    returned: ExpressionHandle,
    condition: ExpressionHandle,
) -> Option<bool> {
    // Domain selection precedes observation construction. Non-IEEE goals use
    // their existing proof owners; do not rebuild a state prefix merely to
    // discover afterward that an integer or structural goal is not ours.
    if !has_float_comparison(program, machine, state, condition) {
        return None;
    }
    let environment =
        super::occurrence_value_environment(program, machine.symbol, state.symbol, returned)?;
    let is_result = |expression| {
        crate::reserved_result_owner(program, expression)
            .is_some_and(|(owner, _)| owner == machine.symbol || owner == state.symbol)
    };
    let actual = |expression| {
        if is_result(expression) {
            returned
        } else {
            expression
        }
    };
    let result_type = |expression| {
        crate::expression_result_type_reference(program, machine, state, actual(expression))
    };
    let source_bounds = |expression, reference| {
        // Anonymous literals land at this selected comparison's operand
        // carrier. This supplies a format, never a bound from the goal. Typed
        // values retain their own format and cannot use this fallback.
        let landed_type = |value| {
            result_type(value).or_else(|| {
                matches!(program.expression_table.peeled_expression(value),
                    typed_trees::expression::ExpressionNode::Float(literal)
                        if literal.landing().is_none())
                .then_some(reference)
            })
        };
        float_source_bounds(
            program,
            machine.symbol,
            &environment,
            actual(expression),
            &landed_type,
        )
    };
    predicate(
        program,
        machine.symbol,
        condition,
        &actual,
        &source_bounds,
        &result_type,
    )
}

fn has_float_comparison(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    if crate::mathematical_applications::float_predicates(program, expression)
        .is_some_and(|predicates| !predicates.is_empty())
    {
        return true;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        return has_float_comparison(program, machine, state, binary.left)
            || has_float_comparison(program, machine, state, binary.right);
    }
    [binary.left, binary.right].into_iter().any(|operand| {
        crate::reserved_result_owner(program, operand)
            .map(|(_, reference)| reference)
            .or_else(|| crate::expression_result_type_reference(program, machine, state, operand))
            .and_then(|reference| program.primitive_type_reference(reference))
            .is_some_and(|primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64))
    })
}

fn predicate(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
    actual: &dyn Fn(ExpressionHandle) -> ExpressionHandle,
    source_bounds: &dyn Fn(ExpressionHandle, TypeReferenceHandle) -> Option<FloatSourceBounds>,
    result_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
) -> Option<bool> {
    if let Some(predicates) =
        crate::mathematical_applications::float_predicates(program, expression)
    {
        let mut unknown = false;
        for projection in predicates {
            let source = source_bounds(projection.subject, projection.reference)?;
            if !matches!(
                (source.primitive, projection.predicate.format()),
                (PrimitiveType::F32, numerics::literals::FloatFormat::F32)
                    | (PrimitiveType::F64, numerics::literals::FloatFormat::F64)
            ) {
                return None;
            }
            let endpoint = projection.bound.to_f64();
            let bound = FloatSourceBounds {
                primitive: source.primitive,
                minimum: endpoint,
                maximum: endpoint,
            };
            match compare(projection.operator, &source, &bound) {
                Some(false) => return Some(false),
                None => unknown = true,
                Some(true) => {}
            }
        }
        return (!unknown).then_some(true);
    }
    let binary = program.expression_table.binary_node(expression)?;
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        let left = predicate(
            program,
            owner,
            binary.left,
            actual,
            source_bounds,
            result_type,
        );
        let right = predicate(
            program,
            owner,
            binary.right,
            actual,
            source_bounds,
            result_type,
        );
        return match (left, right, binary.operator) {
            (Some(false), _, BinaryOperator::And) | (_, Some(false), BinaryOperator::And) => {
                Some(false)
            }
            (Some(true), Some(true), BinaryOperator::And) => Some(true),
            (Some(true), _, BinaryOperator::Or) | (_, Some(true), BinaryOperator::Or) => Some(true),
            (Some(false), Some(false), BinaryOperator::Or) => Some(false),
            _ => None,
        };
    }
    // Comparison authority belongs to the authored operands, including their
    // qualifications. Substitution supplies values, not a different overload.
    let authored_type = |operand| {
        crate::reserved_result_owner(program, operand)
            .map(|(_, reference)| reference)
            .or_else(|| result_type(operand))
    };
    let operand_types = [authored_type(binary.left), authored_type(binary.right)];
    let reference = operand_types[0].or(operand_types[1])?;
    let primitive = program.primitive_type_reference(reference)?;
    if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
        || !comparison_meaning(program, owner, expression, binary.operator, &operand_types)
    {
        return None;
    }
    let left = source_bounds(binary.left, reference)?;
    let right = source_bounds(binary.right, reference)?;
    if left.primitive != right.primitive {
        return None;
    }
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && super::place_path(program, actual(binary.left)).is_some_and(|left_path| {
        super::place_path(program, actual(binary.right)).as_deref() == Some(left_path.as_str())
    }) {
        return Some(binary.operator == BinaryOperator::Equal);
    }
    compare(binary.operator, &left, &right)
}

fn compare(
    operator: BinaryOperator,
    left: &FloatSourceBounds,
    right: &FloatSourceBounds,
) -> Option<bool> {
    use BinaryOperator::*;
    let same_point = left.minimum == left.maximum
        && right.minimum == right.maximum
        && left.minimum == right.minimum;
    let disjoint = left.maximum < right.minimum || right.maximum < left.minimum;
    match operator {
        Equal if same_point => Some(true),
        Equal if disjoint => Some(false),
        NotEqual if disjoint => Some(true),
        NotEqual if same_point => Some(false),
        Less if left.maximum < right.minimum => Some(true),
        Less if left.minimum >= right.maximum => Some(false),
        LessOrEqual if left.maximum <= right.minimum => Some(true),
        LessOrEqual if left.minimum > right.maximum => Some(false),
        Greater => compare(Less, right, left),
        GreaterOrEqual => compare(LessOrEqual, right, left),
        _ => None,
    }
}
