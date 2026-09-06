//! Checked integer contract predicates, separate from execution expressions.

use super::*;

/// The caller supplies a predicate from this machine's exact contract clause.
/// Entry scalar parameters precede the reserved ensures-only result position.
pub(crate) fn lower_integer_contract_predicate(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    allow_result: bool,
) -> Option<CheckedBooleanExpression> {
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);
    if parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const)
    {
        return None;
    }
    let scalar_count = parameters
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .count();
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if operators.uses.iter().any(|(_, operator)| {
        operator.expression == expression
            && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return None;
    }
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        let left = Box::new(lower_integer_contract_predicate(
            program,
            operators,
            machine,
            binary.left,
            allow_result,
        )?);
        let right = Box::new(lower_integer_contract_predicate(
            program,
            operators,
            machine,
            binary.right,
            allow_result,
        )?);
        return Some(if binary.operator == BinaryOperator::And {
            CheckedBooleanExpression::And { left, right }
        } else {
            CheckedBooleanExpression::Or { left, right }
        });
    }
    let subject = |expression| {
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            return None;
        };
        if let Some((position, parameter)) = parameters.iter().enumerate().find(|(_, parameter)| {
            parameter.symbol.is_valid()
                && parameter.symbol == path.symbol
                && path.head_symbol == parameter.symbol
        }) {
            // An ensures name denotes the post-state, not an implicit old(...).
            if allow_result && parameter.is_mutable {
                return None;
            }
            program.primitive_type_reference(parameter.type_reference)?;
            let scalar_position = parameters[..position]
                .iter()
                .filter(|parameter| {
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                })
                .count();
            return Some((scalar_position, parameter.type_reference));
        }
        (allow_result
            && !parameters
                .iter()
                .any(|parameter| parameter.name.as_str() == "result")
            && matches!(program.expression_table.name_path_members(path.members),
                [name] if name.as_str() == "result"))
        .then_some((scalar_count, entry.return_type))
    };
    use language_core::OperatorSpelling;
    let spelling = match binary.operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    };
    let subjects = [binary.left, binary.right].map(subject);
    let operand_types = subjects.map(|subject| subject.map(|(_, type_reference)| type_reference));
    // A named subject supplies contextual literal landing. Literal-only
    // tautologies retain their existing closed-value representation instead.
    let contextual_type = operand_types.into_iter().flatten().next()?;
    let contextual_primitive = program.primitive_type_reference(contextual_type)?;
    if !is_integer(contextual_primitive) {
        return None;
    }
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &operand_types,
    ) {
        return None;
    }
    let operand = |expression| {
        if let Some((position, type_reference)) = subject(expression) {
            let primitive_type = program.primitive_type_reference(type_reference)?;
            if !is_integer(primitive_type) {
                return None;
            }
            return Some(CheckedScalarExpression::Parameter {
                position,
                primitive_type,
            });
        }
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Integer(_)
        ) {
            return None;
        }
        lower_return_expression(
            program,
            operators,
            expression,
            &[],
            &[],
            &[],
            contextual_primitive,
            &[],
        )
    };
    let left = operand(binary.left)?;
    let right = operand(binary.right)?;
    construct_integer_comparison(binary.operator, left, right)
}

/// Bracket constraints are native numeric requires sugar. Keep every present
/// range, including an explicit unsupported row when its endpoints cannot be
/// represented in the bounded scalar contract language.
pub(crate) fn lower_integer_parameter_range_requirements(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
) -> Vec<Option<CheckedBooleanExpression>> {
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let parameters = program.state_parameters(entry);
    let mut requirements = Vec::new();
    let mut scalar_position = 0;
    for parameter in parameters {
        let primitive_type = program.primitive_type_reference(parameter.type_reference);
        let position = scalar_position;
        if primitive_type.is_some() {
            scalar_position += 1;
        }
        let mut type_reference = parameter.type_reference;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        let typed_trees::types::TypeConstraintNode::Range { minimum, maximum } =
                            constraint
                        else {
                            continue;
                        };
                        let predicate = || {
                            // Existing source validation rejects range constraints
                            // outside Exact: those domains do not enforce stores.
                            if parameter.is_self
                                || parameter.is_const
                                || primitive_type.is_none()
                                || program
                                    .arithmetic_domain_for_type_reference(parameter.type_reference)
                                    != ArithmeticDomain::Exact
                            {
                                return None;
                            }
                            let primitive_type =
                                program.primitive_type_reference(parameter.type_reference)?;
                            if !is_integer(primitive_type) {
                                return None;
                            }
                            let expressions = &program.expression_table;
                            let (ExpressionNode::Integer(low), ExpressionNode::Integer(high)) = (
                                expressions.expression(*minimum),
                                expressions.expression(*maximum),
                            ) else {
                                return None;
                            };
                            if low.value_bignum()? > high.value_bignum()? {
                                return None;
                            }
                            let minimum = lower_return_expression(
                                program,
                                operators,
                                *minimum,
                                &[],
                                &[],
                                &[],
                                primitive_type,
                                &[],
                            )?;
                            let maximum = lower_return_expression(
                                program,
                                operators,
                                *maximum,
                                &[],
                                &[],
                                &[],
                                primitive_type,
                                &[],
                            )?;
                            let subject = CheckedScalarExpression::Parameter {
                                position,
                                primitive_type,
                            };
                            // This is the meaning of TypeConstraintNode::Range,
                            // not an authored selectable <= operator occurrence.
                            Some(CheckedBooleanExpression::And {
                                left: Box::new(construct_integer_comparison(
                                    BinaryOperator::LessOrEqual,
                                    minimum,
                                    subject.clone(),
                                )?),
                                right: Box::new(construct_integer_comparison(
                                    BinaryOperator::LessOrEqual,
                                    subject,
                                    maximum,
                                )?),
                            })
                        };
                        requirements.push(predicate());
                    }
                    type_reference = *base_type;
                }
                _ => break,
            }
        }
    }
    requirements
}
