//! Scalar contract predicates share logical structure and exact entry/result
//! identities. They are not body expressions: mutable entry snapshots are
//! available to requires, while normal guarantees cannot reread them as old
//! values. Integer leaves keep their contextual landing and operator owner.
use crate::values::operator_is_builtin;
use crate::values::scalar::boolean_lowering::construct_integer_comparison;
use crate::values::scalar::contract_entry;
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use crate::values::scalar_expression_type;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedOperatorResolutionStatus;
use checked_trees::CheckedScalarExpression;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::UnaryOperator;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

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

/// One authored `requires` predicate on a non-entry state, lowered into the
/// closed scalar contract namespace. The state's authored parameter roster
/// indexes the subjects; no result position exists on an arrival contract.
/// The machine still owns the builtin-meaning checks inside the predicate.
pub(crate) fn lower_state_scalar_contract_predicate(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    remaining: &mut usize,
) -> Option<CheckedBooleanExpression> {
    ContractPredicates {
        program,
        operators,
        machine,
        parameters: program.state_parameters(state),
        allow_result: false,
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
    fn subject(&self, expression: ExpressionHandle) -> Option<(usize, TypeReferenceHandle, bool)> {
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
            // Erased formals index the proof-only roster in authored order.
            if parameter.relevance.is_erased() {
                let erased_position = self.parameters[..position]
                    .iter()
                    .filter(|parameter| {
                        parameter.relevance.is_erased()
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })
                    .count();
                return Some((erased_position, parameter.type_reference, true));
            }
            let scalar_position = self.parameters[..position]
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(program, parameter)
                })
                .count();
            return Some((scalar_position, parameter.type_reference, false));
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
                        crate::values::scalar::occupies_scalar_position(program, parameter)
                    })
                    .count(),
                entry.return_type,
                false,
            )
        })
    }

    /// One comparison operand as a typed scalar term: an exact entry/result
    /// subject, or exact integer arithmetic (`+`, `-`, `*` with builtin
    /// meaning) over such subjects and contextual integer literals. The
    /// carrier must be an exact-domain integer; a wrapping/saturating/
    /// trapping operand or any other spelling stays outside the closed
    /// language and lowers to `None`.
    fn integer_term(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
        let program = self.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => {
                let (position, reference, erased) = self.subject(expression)?;
                let primitive_type = program.primitive_type_reference(reference)?;
                Some((
                    if erased {
                        CheckedScalarExpression::ErasedParameter {
                            position,
                            primitive_type,
                        }
                    } else {
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type,
                        }
                    },
                    reference,
                ))
            }
            ExpressionNode::Binary(binary)
                if depth < 64 && operator_is_builtin(self.operators, expression) =>
            {
                use checked_trees::CheckedIntegerBinaryKind;
                let kind = match binary.operator {
                    BinaryOperator::Add => CheckedIntegerBinaryKind::ExactAdd,
                    BinaryOperator::Subtract => CheckedIntegerBinaryKind::ExactSubtract,
                    BinaryOperator::Multiply => CheckedIntegerBinaryKind::ExactMultiply,
                    _ => return None,
                };
                let terms = [binary.left, binary.right]
                    .map(|operand| self.integer_term(operand, depth + 1));
                // The typed peer supplies the carrier; a literal peer lands on
                // it. Two literals have no carrier and stay unsupported here.
                let (carrier_type, reference) =
                    terms.iter().flatten().find_map(|(term, reference)| {
                        Some((scalar_expression_type(term)?, *reference))
                    })?;
                if !is_integer(carrier_type)
                    || program.arithmetic_domain_for_type_reference(reference)
                        != ArithmeticDomain::Exact
                {
                    return None;
                }
                let operand_types = [Some(reference), Some(reference)];
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    self.machine.symbol,
                    expression,
                    match binary.operator {
                        BinaryOperator::Add => language_core::OperatorSpelling::Add,
                        BinaryOperator::Subtract => language_core::OperatorSpelling::Subtract,
                        _ => language_core::OperatorSpelling::Multiply,
                    },
                    &operand_types,
                ) {
                    return None;
                }
                let [left, right] = [
                    (binary.left, terms[0].clone()),
                    (binary.right, terms[1].clone()),
                ]
                .map(|(operand, term)| match term {
                    Some((term, _)) => {
                        (scalar_expression_type(&term) == Some(carrier_type)).then_some(term)
                    }
                    None => {
                        if !matches!(
                            program.expression_table.expression(operand),
                            ExpressionNode::Integer(_)
                        ) {
                            return None;
                        }
                        lower_return_expression(
                            program,
                            self.operators,
                            operand,
                            &[],
                            &[],
                            &[],
                            &[],
                            carrier_type,
                            &[],
                        )
                    }
                });
                Some((
                    CheckedScalarExpression::IntegerBinary {
                        kind,
                        primitive_type: carrier_type,
                        left: Box::new(left?),
                        right: Box::new(right?),
                    },
                    reference,
                ))
            }
            _ => None,
        }
    }

    fn boolean_type(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) {
            return self.subject(expression).map(|(_, reference, _)| reference);
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
                let (position, reference, erased) = self.subject(expression)?;
                (program.primitive_type_reference(reference) == Some(PrimitiveType::Bool))
                    .then_some(if erased {
                        CheckedBooleanExpression::ErasedParameter { position }
                    } else {
                        CheckedBooleanExpression::Parameter { position }
                    })
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
                let subjects =
                    [binary.left, binary.right].map(|expression| self.integer_term(expression, 0));
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

/// One authored parameter range constraint observed by the scalar contract
/// family. The three projections share a single constraint walk so a
/// requires-tail clause row and its retained roster evidence can never
/// disagree about which source range they describe.
pub(crate) struct ParameterRangeRequirements {
    /// Integer bounds as closed comparison predicates, one slot per authored
    /// range. The crash requirement capsule and the boundary rejoin read this
    /// projection; floating ranges keep an explicit `None` here because they
    /// are not integer predicates.
    pub(crate) integer_predicates: Vec<Option<CheckedBooleanExpression>>,
    /// Requires-tail rows in authored constraint order: supported integer
    /// ranges as `Predicate` clauses and retained floating ranges as
    /// `FloatRange` clauses carrying their IEEE endpoints verbatim. `None`
    /// marks a present range that could not be retained at all.
    pub(crate) scalar_clauses: Vec<Option<checked_trees::ClosedScalarContractValue>>,
    /// The retained floating roster in dense scalar-parameter order. `None`
    /// records an incomplete roster — an authored floating range whose
    /// endpoints could not be retained exactly — so consumers fail closed
    /// rather than read a partial roster as complete.
    pub(crate) float_entry_ranges: Option<Vec<checked_trees::ClosedFloatRangeRequirement>>,
}

/// Bracket constraints are native numeric requires sugar. Keep every present
/// range: integer intervals land as closed `<=` conjunctions while floating
/// windows retain their authored IEEE endpoints and boundary kind verbatim —
/// the exclusive end stays authored, never an integer predecessor.
pub(crate) fn lower_scalar_parameter_range_requirements(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> ParameterRangeRequirements {
    let mut ranges = ParameterRangeRequirements {
        integer_predicates: Vec::new(),
        scalar_clauses: Vec::new(),
        float_entry_ranges: Some(Vec::new()),
    };
    let Some(entry) = program.machine_states(machine).first() else {
        // No entry state means no constraints at all; the roster stays
        // `None` rather than claiming an empty-but-complete floating roster.
        ranges.float_entry_ranges = None;
        return ranges;
    };
    let parameters = program.state_parameters(entry);
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
                        let typed_trees::types::TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } = constraint
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
                            let high = validation::closed_integer_range_maximum(
                                program,
                                *maximum,
                                *end_inclusive,
                            )?;
                            if low > high {
                                return None;
                            }
                            // Endpoints have already been evaluated under their
                            // own selected meaning. Land the normalized interval,
                            // not an exclusive end outside the subject carrier.
                            let minimum = CheckedScalarExpression::IntegerLiteral {
                                literal: validation::land_integer_value(&low, primitive_type)?,
                            };
                            let maximum = CheckedScalarExpression::IntegerLiteral {
                                literal: validation::land_integer_value(&high, primitive_type)?,
                            };
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
                        let requirement = || {
                            // Existing source validation rejects range constraints
                            // outside Exact: those domains do not enforce stores.
                            if parameter.is_self
                                || parameter.is_const
                                || program
                                    .arithmetic_domain_for_type_reference(parameter.type_reference)
                                    != ArithmeticDomain::Exact
                            {
                                return None;
                            }
                            let primitive_type = primitive_type?;
                            let minimum = validation::closed_float_range_endpoint(
                                program,
                                *minimum,
                                primitive_type,
                            )?;
                            let maximum = validation::closed_float_range_endpoint(
                                program,
                                *maximum,
                                primitive_type,
                            )?;
                            // IEEE order: a NaN endpoint or a reversed window
                            // cannot be retained as a nonempty requirement.
                            if !validation::ieee_float_range_ordered(minimum, maximum) {
                                return None;
                            }
                            Some(checked_trees::ClosedFloatRangeRequirement {
                                position,
                                primitive_type,
                                minimum,
                                maximum,
                                maximum_inclusive: *end_inclusive,
                            })
                        };
                        let predicate = predicate();
                        if matches!(
                            primitive_type,
                            Some(PrimitiveType::F32 | PrimitiveType::F64)
                        ) {
                            // A floating range is a `FloatRange` clause in the
                            // requires tail; its retained evidence rides the
                            // roster. One failed endpoint voids the whole
                            // roster so no consumer reads a partial one.
                            match requirement() {
                                Some(requirement) => {
                                    if let Some(roster) = &mut ranges.float_entry_ranges {
                                        roster.push(requirement);
                                    }
                                    ranges.scalar_clauses.push(Some(
                                        checked_trees::ClosedScalarContractValue::FloatRange(
                                            requirement,
                                        ),
                                    ));
                                }
                                None => {
                                    ranges.float_entry_ranges = None;
                                    ranges.scalar_clauses.push(None);
                                }
                            }
                        } else {
                            ranges.scalar_clauses.push(
                                predicate
                                    .clone()
                                    .map(checked_trees::ClosedScalarContractValue::Predicate),
                            );
                        }
                        ranges.integer_predicates.push(predicate);
                    }
                    type_reference = *base_type;
                }
                _ => break,
            }
        }
    }
    ranges
}

/// The integer projection of the authored parameter ranges; floating windows
/// keep an explicit unsupported slot because they are not integer predicates.
pub(crate) fn lower_integer_parameter_range_requirements(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<Option<CheckedBooleanExpression>> {
    lower_scalar_parameter_range_requirements(program, machine).integer_predicates
}
