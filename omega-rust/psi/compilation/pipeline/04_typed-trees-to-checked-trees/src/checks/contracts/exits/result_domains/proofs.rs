//! proofs slice of result_domains.

use validation::canonical_place_from_expression_in_state;

use super::super::super::return_values::exit_return_expression;
use super::{domain_predicate_meaning, exact_integer_reference};
use crate::flow::canonical_place_from_semantic_place;
use checked_trees::{CheckFacts, FlowExitFact};
use facts::{FactContextHandle, FactOrigin, FactPayload};
use typed_trees::types::TypeReferenceNode;

pub(super) fn establishes_scalar_predicates(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    contexts: &[FactContextHandle],
    domain: &typed_trees::domain::DomainDefinition,
    constraint: &typed_trees::types::DomainConstraint,
) -> bool {
    if domain.alias.is_some() {
        return false;
    }
    let Some(indices) = closed_domain_indices(program, domain, constraint) else {
        return false;
    };
    let Some(machine) = program.machine(exit.machine_symbol) else {
        return false;
    };
    let Some(state) = validation::find_state_for_exit(program, exit) else {
        return false;
    };
    let proof = ScalarReturnProof {
        program,
        facts,
        exit,
        contexts,
        machine,
        state,
    };
    let mut hypotheses = contexts
        .iter()
        .flat_map(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
        })
        .filter_map(|fact| {
            // Raw callee contracts do not denote caller values. Their substitution
            // belongs to the call-evidence owner, not to this return's roster.
            if matches!(
                fact.origin,
                FactOrigin::CallRequires | FactOrigin::CallEnsures
            ) {
                return None;
            }
            let (expression, holds) = match fact.payload {
                FactPayload::ContractBooleanExpression {
                    kind: facts::ContractFactKind::Requires,
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => (expression, true),
                FactPayload::BooleanExpression(expression) => (expression, true),
                FactPayload::BooleanValue { expression, value } => (expression, value),
                _ => return None,
            };
            Some(validation::ScopedArithmeticHypothesis {
                proposition: proof.current_expression(expression)?,
                holds,
            })
        })
        .collect::<Vec<_>>();
    // Membership retained at the exit entails its declaration's predicates on
    // that exact live subject. Reading a parameter's static type here would
    // instead resurrect qualifications invalidated by a body mutation.
    for context in contexts {
        for fact in facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
        {
            if matches!(
                fact.origin,
                FactOrigin::CallRequires | FactOrigin::CallEnsures
            ) {
                continue;
            }
            let Some((_, _, domain_symbol, semantic_domain)) = fact.payload.domain_membership()
            else {
                continue;
            };
            let Some(candidate) = program.domain_definition(domain_symbol) else {
                continue;
            };
            if !semantic_domain.is_valid()
                || semantic_domain != candidate.semantic_id
                || !typed_trees::domain::index_parameters(program, candidate).is_empty()
            {
                continue;
            }
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let Some(subject) = canonical_place_from_semantic_place(
                program,
                &facts.semantic,
                facts.semantic.places.get(place),
            ) else {
                continue;
            };
            let Some(primitive) = program
                .primitive_type_reference(candidate.target_type)
                .filter(|primitive| primitive.accepts_integer_literal())
            else {
                continue;
            };
            for predicate in program.proof_facts.span_or_empty(candidate.facts) {
                let Some(expression) = predicate.as_expression() else {
                    continue;
                };
                let mut subjects = Vec::new();
                if domain_predicate_meaning(program, candidate, *expression, &mut subjects, 0)
                    .is_none()
                {
                    continue;
                }
                hypotheses.push(validation::ScopedArithmeticHypothesis {
                    proposition: validation::ScopedArithmeticExpression {
                        owner: candidate.symbol,
                        expression: *expression,
                        bindings: vec![validation::ScopedArithmeticBinding {
                            binder: validation::ScopedArithmeticBinder::Symbol(
                                candidate.subject_symbol,
                            ),
                            value: validation::ScopedArithmeticValue::Atom {
                                identity: format!("return-place:{subject:?}"),
                                unsigned: !primitive.is_signed_integer(),
                            },
                        }],
                    },
                    holds: true,
                });
            }
        }
    }
    program
        .proof_facts
        .span_or_empty(domain.facts)
        .iter()
        .all(|fact| {
            let Some(expression) = fact.as_expression() else {
                // A membership or proposition conjunct remains an obligation; an
                // arithmetic proof of its neighbors cannot discard it.
                return false;
            };
            let returned = exit_return_expression(program, exit);
            if super::super::super::float_predicate_domains::expression_establishes_domain_predicate(
                program,
                facts,
                contexts,
                proof.machine,
                proof.state,
                exit.statement_index,
                returned,
                domain,
                *expression,
            ) {
                return true;
            }
            let mut subjects = Vec::new();
            if domain_predicate_meaning(program, domain, *expression, &mut subjects, 0).is_none() {
                return false;
            }
            let returned = exit_return_expression(program, exit);
            let known = proof.current_value(returned);
            if super::super::super::prover::evaluate_scalar(
                program,
                *expression,
                &mut |leaf| {
                    if subjects.contains(&leaf) {
                        return known.clone();
                    }
                    let typed_trees::expression::ExpressionNode::Name(path) =
                        program.expression_table.expression(leaf)
                    else {
                        return None;
                    };
                    indices
                        .iter()
                        .find(|index| index.symbol == path.symbol)
                        .map(|index| index.value.clone())
                },
                &|_, _| true,
            ) == Some(super::super::super::prover::ScalarValue::Boolean(true))
            {
                return true;
            }
            let Some(returned) = proof.current_expression(returned) else {
                return false;
            };
            let mut bindings = Vec::new();
            for index in &indices {
                let super::super::super::prover::ScalarValue::Integer(value) = &index.value else {
                    return false;
                };
                bindings.push(validation::ScopedArithmeticBinding {
                    binder: validation::ScopedArithmeticBinder::Symbol(index.symbol),
                    value: validation::ScopedArithmeticValue::Integer(value.clone()),
                });
            }
            bindings.push(validation::ScopedArithmeticBinding {
                binder: validation::ScopedArithmeticBinder::Symbol(domain.subject_symbol),
                value: validation::ScopedArithmeticValue::Term(returned),
            });
            let goal = validation::ScopedArithmeticExpression {
                owner: domain.symbol,
                expression: *expression,
                bindings,
            };
            validation::scoped_arithmetic_implication(program, machine, &hypotheses, &goal)
                == validation::StrictArithmeticImplicationJudgment::Proven
        })
}

pub(super) struct ScalarDomainIndex {
    symbol: symbols::SymbolHandle,
    value: super::super::super::prover::ScalarValue,
}

/// A predicate instance is reconstructed from the retained actual arguments,
/// not decoded from an identity label. Ordinary const admission establishes
/// each argument's declared carrier/range before its canonical scalar payload
/// becomes a mathematical substitution. The complete tuple must reproduce the
/// exact semantic instance the result type promises.
pub(super) fn closed_domain_indices(
    program: &dyn ::validation::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    constraint: &typed_trees::types::DomainConstraint,
) -> Option<Vec<ScalarDomainIndex>> {
    use super::super::super::prover::ScalarValue;
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    let parameters = typed_trees::domain::index_parameters(program, domain);
    if parameters.len() != constraint.arguments.len() || !constraint.semantic_id.is_valid() {
        return None;
    }
    let identity = typed_trees::domain::indexed_domain_instance_name(
        program,
        domain,
        parameters,
        &constraint.arguments,
    )
    .ok()?;
    if program.semantic_domains.lookup(&identity) != Some(constraint.semantic_id) {
        return None;
    }
    let mut indices = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(&constraint.arguments) {
        validation::validate_closed_const_argument(
            program,
            domain.name.as_str(),
            parameter,
            *argument,
        )
        .ok()?;
        let TypeReferenceNode::Named { name, .. } =
            program.type_reference_table.type_reference(*argument)
        else {
            return None;
        };
        let value = if let Some(canonical) = CanonicalConstValue::from_atom(name.as_str()) {
            match canonical.decode_encoding()? {
                DecodedCanonicalConstValue::Integer { value, .. } => {
                    ScalarValue::Integer(numerics::bignum::BigInt::from_i128(value))
                }
                DecodedCanonicalConstValue::Boolean(value) => ScalarValue::Boolean(value),
                _ => return None,
            }
        } else {
            ScalarValue::Integer(numerics::bignum::BigInt::from_i128(
                name.as_str().parse::<i128>().ok()?,
            ))
        };
        indices.push(ScalarDomainIndex {
            symbol: parameter.symbol,
            value,
        });
    }
    Some(indices)
}

pub(super) struct ScalarReturnProof<'a> {
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    exit: &'a FlowExitFact,
    contexts: &'a [FactContextHandle],
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
}

impl ScalarReturnProof<'_> {
    /// An exact value conversion within one primitive carrier preserves its
    /// value. This establishes no target qualification: the predicate is
    /// proved again over the source in the same live context. Anonymous
    /// integer literals additionally undergo the ordinary exact landing check.
    pub(super) fn value_preserving_source(
        &self,
        mut expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        for _ in 0..128 {
            let Some(cast) = self.program.expression_table.cast_node(expression) else {
                return Some(expression);
            };
            if cast.form.is_recast() || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
            {
                return None;
            }
            let target = self.program.primitive_type_reference(cast.target_type)?;
            let source = validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                cast.value,
            )
            .and_then(|reference| self.program.primitive_type_reference(reference));
            if source != Some(target) {
                let literal = self.program.expression_table.integer_node(cast.value)?;
                if literal.landing().is_some()
                    || validation::land_integer_value(&literal.value_bignum()?, target).is_none()
                {
                    return None;
                }
            }
            expression = cast.value;
        }
        None
    }

    /// Mathematical evaluation is permitted only for exact integer
    /// computations. Comparisons can still observe policy-bearing values;
    /// their operands are values, not permission to replace wrapping or
    /// saturating operations by unbounded arithmetic.
    pub(super) fn exact_computations(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        depth: usize,
    ) -> bool {
        use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};
        if self
            .program
            .expression_table
            .expression_at_depth(expression, depth)
            .is_none()
        {
            return false;
        }
        let exact_operand = |operand| {
            validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                operand,
            )
            .map(|reference| exact_integer_reference(self.program, reference))
            .unwrap_or_else(|| {
                matches!(self.program.expression_table.expression(operand),
                    ExpressionNode::Integer(literal) if literal.landing().is_none())
            })
        };
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                let observes_values = matches!(
                    binary.operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                        | BinaryOperator::And
                        | BinaryOperator::Or
                );
                (observes_values || (exact_operand(binary.left) && exact_operand(binary.right)))
                    && self.exact_computations(binary.left, depth + 1)
                    && self.exact_computations(binary.right, depth + 1)
            }
            ExpressionNode::Unary(unary) => {
                unary.operator == UnaryOperator::LogicalNot
                    && self.exact_computations(unary.operand, depth + 1)
            }
            ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {
                true
            }
            _ => false,
        }
    }

    pub(super) fn current_expression(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<validation::ScopedArithmeticExpression> {
        let expression = self.value_preserving_source(expression)?;
        if !self.exact_computations(expression, 0)
            || !validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            )
        {
            return None;
        }
        let mut bindings = Vec::new();
        self.current_bindings(expression, &mut bindings, 0)?;
        Some(validation::ScopedArithmeticExpression {
            owner: self.state.symbol,
            expression,
            bindings,
        })
    }

    pub(super) fn current_bindings(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        bindings: &mut Vec<validation::ScopedArithmeticBinding>,
        depth: usize,
    ) -> Option<()> {
        use typed_trees::expression::ExpressionNode;
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
            ExpressionNode::Name(path)
                if path.symbol.is_valid() && path.head_symbol == path.symbol =>
            {
                let reference = validation::declared_place_type_raw(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                )?;
                let primitive = self.program.primitive_type_reference(reference)?;
                if !primitive.accepts_integer_literal() {
                    return None;
                }
                let subject = canonical_place_from_expression_in_state(
                    self.program,
                    self.exit.state_symbol,
                    self.exit.statement_index,
                    expression,
                )?;
                let value = crate::values::literal_at_place(
                    self.program,
                    &self.facts.semantic,
                    self.contexts
                        .iter()
                        .map(|context| self.facts.semantic.contexts.get(*context)),
                    &subject,
                )
                .map(|literal| {
                    validation::ScopedArithmeticValue::Term(
                        validation::ScopedArithmeticExpression {
                            owner: self.state.symbol,
                            expression: literal,
                            bindings: Vec::new(),
                        },
                    )
                })
                .unwrap_or_else(|| validation::ScopedArithmeticValue::Atom {
                    identity: format!("return-place:{subject:?}"),
                    unsigned: !primitive.is_signed_integer(),
                });
                bindings.push(validation::ScopedArithmeticBinding {
                    binder: validation::ScopedArithmeticBinder::Symbol(path.symbol),
                    value,
                });
            }
            ExpressionNode::Binary(binary) => {
                self.current_bindings(binary.left, bindings, depth + 1)?;
                self.current_bindings(binary.right, bindings, depth + 1)?;
            }
            ExpressionNode::Unary(unary) => {
                self.current_bindings(unary.operand, bindings, depth + 1)?
            }
            _ => return None,
        }
        Some(())
    }

    pub(super) fn current_value(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<super::super::super::prover::ScalarValue> {
        let expression = self.value_preserving_source(expression)?;
        if !self.exact_computations(expression, 0)
            || !validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            )
        {
            return None;
        }
        super::super::super::prover::evaluate_scalar(
            self.program,
            expression,
            &mut |leaf| {
                let subject = canonical_place_from_expression_in_state(
                    self.program,
                    self.exit.state_symbol,
                    self.exit.statement_index,
                    leaf,
                )?;
                super::super::super::prover::scalar_value_at_place(
                    self.program,
                    &self.facts.semantic,
                    self.contexts
                        .iter()
                        .map(|context| self.facts.semantic.contexts.get(*context)),
                    &subject,
                )
            },
            &|_, _| true,
        )
    }
}

/// Rebase an ensures fact rooted at the reserved `result` occurrence onto the
/// returned expression's canonical place, keeping the fact's own declared
/// field coordinates. `result.bytes` on a `rows[0]` return is exactly
/// `rows[0].bytes` at this statement -- the same place mutation invalidation
/// retires, so corrupted evidence cannot satisfy it.
pub(in crate::checks::contracts::exits) fn proves_result_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    contexts: &[FactContextHandle],
    requirement: &facts::Fact,
) -> bool {
    let domain_symbol = match requirement.payload {
        facts::domain_membership_symbol_pattern!(domain_symbol) => domain_symbol,
        _ => return false,
    };
    let Some(place) = requirement.place.as_place() else {
        return false;
    };
    let place = facts.semantic.places.get(place);
    let Some(root) = place.root.as_expression() else {
        return false;
    };
    // Only the reserved `result` occurrence owned by THIS machine's ensures
    // rebases onto the returned expression. Other expression roots keep their
    // identity (an authored `result` parameter or a foreign occurrence is not
    // the contract result).
    if validation::reserved_result_owner(program, root)
        .is_none_or(|(owner, _)| owner != exit.machine_symbol)
    {
        return false;
    }
    let returned = exit_return_expression(program, exit);
    if !returned.is_valid() {
        return false;
    }
    let Some(mut subject) = canonical_place_from_expression_in_state(
        program,
        exit.state_symbol,
        exit.statement_index,
        returned,
    ) else {
        return false;
    };
    if !crate::flow::place_cases_are_selected(
        program,
        &facts.semantic,
        contexts,
        exit.machine_symbol,
        exit.state_symbol,
        exit.statement_index,
        &subject,
    ) {
        return false;
    }
    subject.extend_segments(facts.semantic.place_segments.span_or_empty(place.segments));
    super::super::super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        contexts,
        &subject,
        domain_symbol,
    )
}
