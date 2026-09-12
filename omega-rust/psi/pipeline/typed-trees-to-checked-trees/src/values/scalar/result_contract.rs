//! Scalar contract predicates share logical structure and exact entry/result
//! identities. They are not body expressions: mutable entry snapshots are
//! available to requires, while normal guarantees cannot reread them as old
//! values. Integer leaves keep their contextual landing and operator owner.

use super::*;

/// The caller supplies a predicate from this machine's exact contract clause.
/// Entry scalar parameters precede the reserved ensures-only result position.
/// The caller owns the shared work budget, including unsuccessful reads.
pub(crate) fn lower_scalar_contract_predicate(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    allow_result: bool,
    remaining: &mut usize,
) -> Option<CheckedBooleanExpression> {
    ContractPredicates {
        program,
        operators,
        machine,
        parameters: contract_entry::authored_entry_parameters(program, machine)?,
        allow_result,
        remaining,
    }
    .boolean(expression, 0)
}

struct ContractPredicates<'program, 'budget> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    machine: &'program typed_trees::machine::Machine,
    parameters: &'program [StateParameter],
    allow_result: bool,
    remaining: &'budget mut usize,
}

impl ContractPredicates<'_, '_> {
    fn subject(&self, expression: ExpressionHandle) -> Option<(usize, TypeReferenceHandle)> {
        let program = self.program;
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            return None;
        };
        if let Some((position, parameter)) =
            self.parameters.iter().enumerate().find(|(_, parameter)| {
                parameter.symbol == path.symbol && path.head_symbol == parameter.symbol
            })
        {
            if parameter.is_self
                || parameter.is_const
                || !matches!(program.expression_table.name_path_members(path.members), [name] if name == &parameter.name)
            {
                return None;
            }
            // An ensures name denotes the post-state, not an implicit old(...).
            if self.allow_result && parameter.is_mutable {
                return None;
            }
            program.primitive_type_reference(parameter.type_reference)?;
            let scalar_position = self.parameters[..position]
                .iter()
                .filter(|parameter| {
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                })
                .count();
            return Some((scalar_position, parameter.type_reference));
        }
        // Equal spelling or carrier does not establish result ownership. This
        // occurrence must belong to this machine's exact authored ensures.
        let entry = program.machine_states(self.machine).first()?;
        (self.allow_result
            && validation::reserved_result_owner(program, expression)
                == Some((self.machine.symbol, entry.return_type)))
        .then(|| {
            (
                self.parameters
                    .iter()
                    .filter(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                    })
                    .count(),
                entry.return_type,
            )
        })
    }

    fn boolean_type(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) {
            return self.subject(expression).map(|(_, reference)| reference);
        }
        self.program
            .type_reference_table
            .named_references()
            .find_map(|(reference, symbol, _)| {
                (self.program.symbols.builtin_type_atom(symbol)
                    == Some(symbols::BuiltinTypeAtom::Bool))
                .then_some(reference)
            })
    }

    fn boolean(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        if depth >= 64
            || *self.remaining == 0
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        *self.remaining -= 1;
        let program = self.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Name(_) => {
                let (position, reference) = self.subject(expression)?;
                (program.primitive_type_reference(reference) == Some(PrimitiveType::Bool))
                    .then_some(CheckedBooleanExpression::Parameter { position })
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                Some(CheckedBooleanExpression::Not(Box::new(
                    self.boolean(unary.operand, depth + 1)?,
                )))
            }
            ExpressionNode::Binary(binary) if operator_is_builtin(self.operators, expression) => {
                let subjects = [binary.left, binary.right].map(|expression| {
                    let (position, reference) = self.subject(expression)?;
                    Some((
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type: program.primitive_type_reference(reference)?,
                        },
                        reference,
                    ))
                });
                if let Some(comparison) = lower_integer_contract_comparison(
                    program,
                    self.operators,
                    self.machine.symbol,
                    expression,
                    subjects,
                ) {
                    return Some(comparison);
                }
                if !matches!(
                    binary.operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                ) {
                    return None;
                }
                let left = Box::new(self.boolean(binary.left, depth + 1)?);
                let right = Box::new(self.boolean(binary.right, depth + 1)?);
                Some(match binary.operator {
                    BinaryOperator::And => CheckedBooleanExpression::And { left, right },
                    BinaryOperator::Or => CheckedBooleanExpression::Or { left, right },
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        let spelling = if binary.operator == BinaryOperator::Equal {
                            language_core::OperatorSpelling::Equal
                        } else {
                            language_core::OperatorSpelling::NotEqual
                        };
                        // Children have been checked as Boolean predicates. Keep
                        // exact formal types for overload selection; the reserved
                        // result is not an unknown wildcard operand.
                        let types = [
                            Some(self.boolean_type(binary.left)?),
                            Some(self.boolean_type(binary.right)?),
                        ];
                        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            self.machine.symbol,
                            expression,
                            spelling,
                            &types,
                        ) {
                            return None;
                        }
                        let equality = CheckedBooleanExpression::Equal { left, right };
                        if binary.operator == BinaryOperator::Equal {
                            equality
                        } else {
                            CheckedBooleanExpression::Not(Box::new(equality))
                        }
                    }
                    _ => return None,
                })
            }
            _ => None,
        }
    }
}

/// Subject readers retain their own namespace custody. Comparison selection,
/// contextual literal landing, and same-carrier construction have one owner.
pub(super) fn lower_integer_contract_comparison(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
    subjects: [Option<(CheckedScalarExpression, TypeReferenceHandle)>; 2],
) -> Option<CheckedBooleanExpression> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if operators.uses.iter().any(|(_, operator)| {
        operator.expression == expression
            && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return None;
    }
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
    let operand_types =
        [(binary.left, &subjects[0]), (binary.right, &subjects[1])].map(|(expression, subject)| {
            subject
                .as_ref()
                .map(|(_, type_reference)| *type_reference)
                .or_else(|| validation::landed_integer_literal_type_reference(program, expression))
        });
    // A typed subject or already-landed literal supplies contextual landing.
    // Keep literal comparisons as predicates too: folding their value here
    // would lose the selected operator and carrier needed by source replay.
    let contextual_type = operand_types.into_iter().flatten().next()?;
    let contextual_primitive = program.primitive_type_reference(contextual_type)?;
    if !is_integer(contextual_primitive) {
        return None;
    }
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        owner,
        expression,
        spelling,
        &operand_types,
    ) {
        return None;
    }
    let operand = |expression, subject: Option<(CheckedScalarExpression, TypeReferenceHandle)>| {
        if let Some((value, type_reference)) = subject {
            let primitive_type = program.primitive_type_reference(type_reference)?;
            if !is_integer(primitive_type) || scalar_expression_type(&value) != Some(primitive_type)
            {
                return None;
            }
            return Some(value);
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
            &[],
            contextual_primitive,
            &[],
        )
    };
    let [left_subject, right_subject] = subjects;
    let left = operand(binary.left, left_subject)?;
    let right = operand(binary.right, right_subject)?;
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
                            let low = validation::closed_integer_range_bound(program, *minimum)?;
                            let high = validation::closed_integer_range_bound(program, *maximum)?;
                            if low > high {
                                return None;
                            }
                            let minimum = lower_return_expression(
                                program,
                                operators,
                                *minimum,
                                &[],
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
