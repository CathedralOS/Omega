//! Modular result bounds from exact, builtin normal-return contract facts.

use super::*;
use language_core::OperatorSpelling;

pub(super) fn normal_return_interval(
    program: &TypedTrees,
    machine: &Machine,
    entry: &State,
    declared: Interval,
) -> Interval {
    // Only gated builtin preconditions may refine a formal used in a result
    // relation. The ordinary source guard environment is not selection evidence.
    let mut environment = ValueEnv::new();
    for parameter in program.state_parameters(entry) {
        if parameter.is_self || parameter.is_mutable || parameter.is_const {
            continue;
        }
        let Some(mut interval) = program
            .primitive_type_reference(parameter.type_reference)
            .and_then(primitive_range)
        else {
            continue;
        };
        for contract in program
            .machine_contracts(machine)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(expression) = fact {
                    interval = interval.intersect(project(
                        program,
                        machine,
                        entry,
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
        .machine_contracts(machine)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .fold(declared, |interval, expression| {
            interval.intersect(project(
                program,
                machine,
                entry,
                &environment,
                expression,
                None,
            ))
        })
}

fn project(
    program: &TypedTrees,
    machine: &Machine,
    entry: &State,
    environment: &ValueEnv,
    expression: ExpressionHandle,
    formal: Option<&typed_trees::signature::StateParameter>,
) -> Interval {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return Interval::UNBOUNDED;
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        // These two selective connectives have no declared operator spelling.
        let left = project(program, machine, entry, environment, binary.left, formal);
        let right = project(program, machine, entry, environment, binary.right, formal);
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
            crate::proof_embeddings::reserved_result_owner(program, expression).is_some_and(
                |(owner, return_type)| owner == machine.symbol && return_type == entry.return_type,
            )
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
        operand_interval(program, entry, environment, other, formal.is_none())
    else {
        return Interval::UNBOUNDED;
    };
    let subject_type = formal.map_or(entry.return_type, |parameter| parameter.type_reference);
    let operand_types = if subject_on_left {
        [Some(subject_type), operand_type]
    } else {
        [operand_type, Some(subject_type)]
    };
    if !builtin(
        program,
        machine,
        expression,
        binary.operator,
        &operand_types,
    ) {
        return Interval::UNBOUNDED;
    }
    guard_narrowing::comparison_interval(binary.operator, operand, subject_on_left)
}

fn operand_interval(
    program: &TypedTrees,
    entry: &State,
    environment: &ValueEnv,
    expression: ExpressionHandle,
    allow_formals: bool,
) -> Option<(Interval, Option<TypeReferenceHandle>)> {
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
        // Unknown literal typing is a wildcard in candidate lookup, not a
        // guessed same-carrier type that could hide a heterogeneous operator.
        return Some((literal_interval(literal), None));
    }
    if !allow_formals {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let parameter = program
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)?;
    if parameter.is_self || parameter.is_mutable || parameter.is_const {
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
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let spelling = match operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return false,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        operand_types,
    )
}
