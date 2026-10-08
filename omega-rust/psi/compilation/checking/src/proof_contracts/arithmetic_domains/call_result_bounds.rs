//! Modular result bounds from exact, builtin normal-return contract facts.
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Interval, SignatureContractKind,
    TypeReferenceHandle, ValueEnvironment, guard_narrowing, range_constraint_interval,
};
use crate::machine_calls::calls::CalleeContract;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    literal_interval, primitive_range,
};
use symbols::SymbolHandle;

pub(super) fn normal_return_interval(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    declared: Interval,
    actuals: &[(SymbolHandle, Interval)],
) -> Interval {
    // Only gated builtin preconditions may refine a formal used in a result
    // relation. The ordinary source guard environment is not selection evidence.
    let mut environment = ValueEnvironment::new();
    for parameter in callee.parameters {
        if parameter.is_mutable || parameter.is_const {
            continue;
        }
        let Some(mut interval) = program
            .primitive_type_reference(parameter.type_reference)
            .and_then(primitive_range)
        else {
            continue;
        };
        if let Some((_, actual)) = actuals
            .iter()
            .find(|(symbol, _)| *symbol == parameter.symbol)
        {
            interval = interval.intersect(*actual);
        }
        for contract in callee
            .contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    interval = interval.intersect(project(
                        program,
                        callee,
                        &environment,
                        *expression,
                        Some(parameter),
                    ));
                }
            }
        }
        environment.set(parameter.name.as_str().to_owned(), interval);
    }
    program
        .contract_expressions(callee.contracts.iter(), SignatureContractKind::Ensures)
        .fold(declared, |interval, expression| {
            interval.intersect(project(program, callee, &environment, expression, None))
        })
}

fn project(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    formal: Option<&typed_trees::signature::StateParameter>,
) -> Interval {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return Interval::UNBOUNDED;
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        // These two selective connectives have no declared operator spelling.
        let left = project(program, callee, environment, binary.left, formal);
        let right = project(program, callee, environment, binary.right, formal);
        return if binary.operator == BinaryOperator::And {
            left.intersect(right)
        } else {
            left.union(right)
        };
    }
    let result = |expression| {
        if let Some(parameter) = formal {
            matches!(program.expression_table.expression(expression), ExpressionNode::Name(path)
            if parameter.symbol.is_valid() && path.symbol == parameter.symbol && path.head_symbol == parameter.symbol)
        } else {
            crate::reserved_result_owner(program, expression).is_some_and(|(owner, return_type)| {
                owner == callee.callable && return_type == callee.return_type
            })
        }
    };
    let (other, subject_on_left) = if result(binary.left) {
        (binary.right, true)
    } else if result(binary.right) {
        (binary.left, false)
    } else {
        return Interval::UNBOUNDED;
    };
    let Some((operand, operand_type)) =
        operand_interval(program, callee, environment, other, formal.is_none())
    else {
        return Interval::UNBOUNDED;
    };
    let subject_type = formal.map_or(callee.return_type, |parameter| parameter.type_reference);
    let operand_types = if subject_on_left {
        [Some(subject_type), operand_type]
    } else {
        [operand_type, Some(subject_type)]
    };
    if !builtin(
        program,
        callee.callable,
        expression,
        binary.operator,
        &operand_types,
    ) {
        return Interval::UNBOUNDED;
    }
    guard_narrowing::comparison_interval(binary.operator, operand, subject_on_left)
}

fn operand_interval(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    allow_formals: bool,
) -> Option<(Interval, Option<TypeReferenceHandle>)> {
    if let Some(cast) = program.expression_table.cast_node(expression) {
        // A total Exact widening preserves the actual's value. Other casts
        // may change it or require their own evidence; their result carrier
        // alone cannot establish a relational guarantee about the source.
        if !crate::proof_contracts::arithmetic_domains::cast_ranges::plain_exact_widening_cast(
            program, cast,
        ) {
            return None;
        }
        let (interval, source_type) =
            operand_interval(program, callee, environment, cast.value, allow_formals)?;
        let source = program.primitive_type_reference(source_type?)?;
        let target = program.primitive_type_reference(cast.target_type)?;
        return crate::integer_widen_is_total(source, target)
            .then_some((interval, Some(cast.target_type)));
    }
    if let Some(literal) = program.expression_table.integer_node(expression) {
        // Unknown literal typing is a wildcard in candidate lookup, not a
        // guessed same-carrier type that could hide a heterogeneous operator.
        return Some((literal_interval(literal), None));
    }
    if !allow_formals {
        return None;
    }
    let path = program.expression_table.name_path_node(expression)?;
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let parameter = callee
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)?;
    if parameter.is_mutable || parameter.is_const {
        return None;
    }
    let primitive = program.primitive_type_reference(parameter.type_reference)?;
    let mut interval = primitive_range(primitive)?;
    if let Some(declared) = range_constraint_interval(program, parameter.type_reference) {
        interval = interval.intersect(declared);
    }
    // Only immutable formal values are read here. Caller expressions and places
    // are never re-evaluated to invent a post-call snapshot.
    if let Some(required) = environment.get(parameter.name.as_str()) {
        interval = interval.intersect(required);
    }
    Some((interval, Some(parameter.type_reference)))
}

fn builtin(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let Some(spelling) = typed_trees::operator::comparison_operator_spelling(operator) else {
        return false;
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        scope,
        expression,
        spelling,
        operand_types,
    )
}
