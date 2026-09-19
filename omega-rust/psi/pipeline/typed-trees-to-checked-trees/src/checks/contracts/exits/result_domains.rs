//! Declared domain obligations at a machine's normal return. A scalar result
//! consumes live membership with its exact domain instance or must independently
//! establish the declared predicates and any authorized provenance. Its result
//! annotation is never an input fact. An `ensures`
//! domain fact authored over the reserved `result` name is discharged against
//! the exact returned expression's live place; a nominal return type
//! additionally makes every declared field qualification an obligation at each
//! value-returning exit, and a readable reference return owes them on
//! the returned place. The return is also a default-domain consumption point
//! for every readable `&mut` referent the machine received, `self`
//! included: the declared field facts assumed on entry must hold again when
//! the referent is handed back. All of these consume only live evidence: a
//! write that retired a field's fact fails the proof, so a nominal
//! annotation alone restores nothing.

use checked_trees::{CheckFacts, FlowExitFact};
use diagnostics::Diagnostic;
use facts::{FactContextHandle, FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint};
use typed_trees::types::TypeReferenceNode;

use super::super::return_values::exit_return_expression;
use crate::flow::{
    CanonicalPlace, canonical_place_from_expression_in_state, canonical_place_from_semantic_place,
};

/// Shared by the exit census and its consumer: a scalar domain annotation
/// needs a live return context even when there is no authored `ensures`.
pub(crate) fn scalar_result_domains(
    program: &typed_trees::TypedTrees,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> Vec<&typed_trees::types::DomainConstraint> {
    // Reference and nominal field obligations retain their existing owners.
    // All primitive scalar carriers participate, not only integer predicates.
    let mut carrier = return_type;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier)
    {
        carrier = *base_type;
    }
    if matches!(
        program.type_reference_table.type_reference(carrier),
        TypeReferenceNode::Reference { .. }
    ) || program.primitive_type_reference(carrier).is_none()
    {
        return Vec::new();
    }
    let mut domains = Vec::new();
    let mut reference = return_type;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        domains.extend(
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    typed_trees::types::TypeConstraintNode::Domain(domain)
                        if domain.symbol.is_valid() =>
                    {
                        Some(domain)
                    }
                    _ => None,
                }),
        );
        reference = *base_type;
    }
    domains
}

/// A scalar signature promises membership to callers, but supplies no evidence
/// to its own body. Consume the returned subject's live exit facts before
/// attempting fresh establishment. In particular, a declaration symbol alone
/// cannot identify an indexed domain application, and predicate truth alone
/// cannot establish a routed qualification.
pub(in crate::checks::contracts) fn check_scalar_result_domains(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == exit.machine_symbol)
    else {
        return;
    };
    let domains = scalar_exit_domains(program, machine, exit.state_symbol);
    if domains.is_empty() {
        return;
    }
    let returned = exit_return_expression(program, exit);
    let contexts = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>();
    let subject = returned
        .is_valid()
        .then(|| {
            canonical_place_from_expression_in_state(
                program,
                exit.state_symbol,
                exit.statement_index,
                returned,
            )
        })
        .flatten();
    if subject.as_ref().is_some_and(|subject| {
        !crate::flow::place_cases_are_selected(
            program,
            &facts.semantic,
            &contexts,
            exit.machine_symbol,
            exit.state_symbol,
            exit.statement_index,
            subject,
        )
    }) {
        diagnostics.push(Diagnostic::error(
            "cannot prove selected case for qualified scalar return",
        ));
        return;
    }
    for constraint in domains {
        let domain_symbol = constraint.symbol;
        let semantic_domain = constraint.semantic_id;
        // Empty theories add no establishment obligation, including indexed
        // instances and aliases whose constituents are all empty theories.
        // Reuse the same rule as checked qualification casts; index identity
        // remains significant when transporting nonvacuous membership below.
        if crate::facts::domain_is_vacuous(program, domain_symbol, &mut Vec::new()) {
            continue;
        }
        let transported = subject.as_ref().is_some_and(|subject| {
            exact_scalar_membership(
                program,
                facts,
                &contexts,
                subject,
                domain_symbol,
                semantic_domain,
            )
        });
        // No actual value means no subject on which to establish a nonempty
        // theory or routed provenance. A signature does not synthesize zero.
        let satisfied = returned.is_valid() && (transported
            || exact_call_result_membership(program, facts, exit, returned, constraint)
            || program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == domain_symbol)
            .is_some_and(|domain| {
                // A route identifying the declaration does not by itself
                // identify any of its indexed applications.
                (typed_trees::domain::index_parameters(program, domain).is_empty()
                    || domain.establishment_routes.is_empty())
                    && (domain.establishment_routes.is_empty()
                        || crate::facts::qualification_evidence::machine_has_checked_domain_establishment(
                            program,
                            machine,
                            domain_symbol,
                        ))
                    && establishes_scalar_predicates(program, facts, exit, &contexts, domain, constraint)
            }));
        if !satisfied {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove scalar result domain for return from {} at statement {}: {}",
                crate::labels::machine_name(program, exit.machine_symbol),
                exit.statement_index,
                crate::labels::symbol_name(program, domain_symbol),
            )));
        }
    }
}

/// Both the machine's external result and the current state's result are
/// obligations. Identical instances need only one establishment judgment.
fn scalar_exit_domains<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_symbol: symbols::SymbolHandle,
) -> Vec<&'program typed_trees::types::DomainConstraint> {
    let mut domains = Vec::new();
    for state in program.machine_states(machine).iter().filter(|state| {
        state.symbol == state_symbol
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
    }) {
        for constraint in scalar_result_domains(program, state.return_type) {
            if !domains
                .iter()
                .any(|existing: &&typed_trees::types::DomainConstraint| {
                    existing.symbol == constraint.symbol
                        && existing.semantic_id == constraint.semantic_id
                })
            {
                domains.push(constraint);
            }
        }
    }
    domains
}

/// A named external tail invocation supplies the returned value directly.
/// Consume its retained call occurrence and checked result contract; it has
/// no authored value expression to manufacture for the ordinary exit prover.
/// Transfers within this machine preserve additional state promises through
/// the selected target contract. The common machine promise is checked at leaf
/// exits, so intermediate states need not repeat it in their annotations.
pub(in crate::checks::contracts) fn check_scalar_tail_result_domains(
    program: &typed_trees::TypedTrees,
    state_flow: &checked_trees::FlowStateFact,
    call: &checked_trees::FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(crate::semantic_calls::CallSite::TransitionNamed { path, .. }) =
        crate::semantic_calls::find_call_site(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )
    else {
        return;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return;
    };
    if !matches!(program.statement_table.statements(state.statement_nodes).get(call.statement_index),
        Some(typed_trees::statement::StatementNode::Transition(transition))
        if transition.exit == typed_trees::statement::TransitionExit::Ordinary)
    {
        return;
    }
    let selected = crate::proof::contract_target_from_state_symbol(program, call.target_symbol);
    let exact_invocation = selected.is_some()
        && crate::proof::contract_target_from_state_symbol(program, path.symbol) == selected;
    let local_transfer =
        exact_invocation && selected.is_some_and(|(owner, _)| owner == machine.symbol);
    let common_domains = program
        .machine_states(machine)
        .first()
        .map(|entry| scalar_result_domains(program, entry.return_type))
        .unwrap_or_default();
    for required in scalar_exit_domains(program, machine, state.symbol) {
        if local_transfer
            && common_domains.iter().any(|common| {
                common.symbol == required.symbol && common.semantic_id == required.semantic_id
            })
        {
            continue;
        }
        if crate::facts::domain_is_vacuous(program, required.symbol, &mut Vec::new()) {
            continue;
        }
        if exact_invocation && call_result_has_domain(program, call.target_symbol, required) {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove scalar result domain for tail return from {} at statement {}: {}",
            crate::labels::machine_name(program, machine.symbol),
            call.statement_index,
            crate::labels::symbol_name(program, required.symbol),
        )));
    }
}

fn call_result_has_domain(
    program: &typed_trees::TypedTrees,
    target: symbols::SymbolHandle,
    required: &typed_trees::types::DomainConstraint,
) -> bool {
    let target = crate::proof::contract_target_from_state_symbol(program, target)
        .map_or(target, |(_, state)| state);
    crate::flow::call_target_return_type(program, target).is_some_and(|return_type| {
        required.semantic_id.is_valid()
            && scalar_result_domains(program, return_type)
                .iter()
                .any(|provided| {
                    provided.symbol == required.symbol
                        && provided.semantic_id == required.semantic_id
                })
    })
}

/// A completed call returns its checked signature's qualification. Rejoin the
/// actual returned expression to the retained invocation occurrence before
/// reading that signature: neither an arbitrary callable symbol nor this
/// enclosing machine's promised result constitutes an invocation result.
fn exact_call_result_membership(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    returned: typed_trees::expression::ExpressionHandle,
    required: &typed_trees::types::DomainConstraint,
) -> bool {
    let typed_trees::expression::ExpressionNode::Call(authored) =
        program.expression_table.expression(returned)
    else {
        return false;
    };
    let mut occurrences = facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == exit.machine_symbol && state.state_symbol == exit.state_symbol
        })
        .flat_map(|(_, state)| facts.flow.control.calls.span_or_empty(state.calls))
        .filter(|call| {
            call.authored_expression == returned && call.statement_index == exit.statement_index
        });
    let Some(call) = occurrences.next() else {
        return false;
    };
    if occurrences.next().is_some() || call.target_symbol != authored.target_symbol {
        return false;
    }
    let Some(crate::semantic_calls::CallSite::Expression {
        expression,
        call: selected,
    }) = crate::semantic_calls::find_call_site(
        program,
        exit.machine_symbol,
        exit.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )
    else {
        return false;
    };
    if expression != returned || selected.target_symbol != call.target_symbol {
        return false;
    }
    call_result_has_domain(program, call.target_symbol, required)
}

pub(in crate::checks::contracts) fn exact_scalar_membership(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain_symbol: symbols::SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
) -> bool {
    semantic_domain.is_valid()
        && contexts.iter().any(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    let (FactPayload::DomainMembership {
                        domain_symbol: candidate,
                        semantic_domain: instance,
                        ..
                    }
                    | FactPayload::ContractDomainMembership {
                        domain_symbol: candidate,
                        semantic_domain: instance,
                        ..
                    }) = fact.payload
                    else {
                        return false;
                    };
                    let FactPlace::Place(place) = fact.place else {
                        return false;
                    };
                    candidate == domain_symbol
                        && instance == semantic_domain
                        && canonical_place_from_semantic_place(
                            program,
                            &facts.semantic,
                            facts.semantic.places.get(place),
                        )
                        .is_some_and(|candidate| candidate == *subject)
                })
        })
}

fn establishes_scalar_predicates(
    program: &typed_trees::TypedTrees,
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
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == exit.machine_symbol)
    else {
        return false;
    };
    let Some(state) = crate::semantic_calls::find_state_in_machine(
        program,
        exit.machine_symbol,
        exit.state_symbol,
    ) else {
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
            let (FactPayload::DomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            }) = fact.payload
            else {
                continue;
            };
            let Some(candidate) = program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == domain_symbol)
            else {
                continue;
            };
            if !semantic_domain.is_valid()
                || semantic_domain != candidate.semantic_id
                || !typed_trees::domain::index_parameters(program, candidate).is_empty()
            {
                continue;
            }
            let FactPlace::Place(place) = fact.place else {
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
                let typed_trees::domain::ProofFact::Expression(expression) = predicate else {
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
                        expression: *expression,
                        bindings: subjects
                            .into_iter()
                            .map(|expression| validation::ScopedArithmeticBinding {
                                binder: validation::ScopedArithmeticBinder::DomainSelf(expression),
                                value: validation::ScopedArithmeticValue::Atom {
                                    identity: format!("return-place:{subject:?}"),
                                    unsigned: !primitive.is_signed_integer(),
                                },
                            })
                            .collect(),
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
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                // A membership or proposition conjunct remains an obligation; an
                // arithmetic proof of its neighbors cannot discard it.
                return false;
            };
            let mut subjects = Vec::new();
            if domain_predicate_meaning(program, domain, *expression, &mut subjects, 0).is_none() {
                return false;
            }
            let returned = exit_return_expression(program, exit);
            let known = proof.current_value(returned);
            if super::super::prover::evaluate_scalar(program, *expression, &mut |leaf| {
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
            }) == Some(super::super::prover::ScalarValue::Boolean(true))
            {
                return true;
            }
            let Some(returned) = proof.current_expression(returned) else {
                return false;
            };
            let mut bindings = Vec::new();
            for index in &indices {
                let super::super::prover::ScalarValue::Integer(value) = &index.value else {
                    return false;
                };
                bindings.push(validation::ScopedArithmeticBinding {
                    binder: validation::ScopedArithmeticBinder::Symbol(index.symbol),
                    value: validation::ScopedArithmeticValue::Integer(value.clone()),
                });
            }
            bindings.extend(subjects.into_iter().map(|subject| {
                validation::ScopedArithmeticBinding {
                    binder: validation::ScopedArithmeticBinder::DomainSelf(subject),
                    value: validation::ScopedArithmeticValue::Term(returned.clone()),
                }
            }));
            let goal = validation::ScopedArithmeticExpression {
                expression: *expression,
                bindings,
            };
            validation::scoped_arithmetic_implication(program, machine, &hypotheses, &goal)
                == validation::StrictArithmeticImplicationJudgment::Proven
        })
}

struct ScalarDomainIndex {
    symbol: symbols::SymbolHandle,
    value: super::super::prover::ScalarValue,
}

/// A predicate instance is reconstructed from the retained actual arguments,
/// not decoded from an identity label. Ordinary const admission establishes
/// each argument's declared carrier/range before its canonical scalar payload
/// becomes a mathematical substitution. The complete tuple must reproduce the
/// exact semantic instance the result type promises.
fn closed_domain_indices(
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::domain::DomainDefinition,
    constraint: &typed_trees::types::DomainConstraint,
) -> Option<Vec<ScalarDomainIndex>> {
    use super::super::prover::ScalarValue;
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

struct ScalarReturnProof<'a> {
    program: &'a typed_trees::TypedTrees,
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
    fn value_preserving_source(
        &self,
        mut expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        use typed_trees::expression::ExpressionNode;
        for _ in 0..128 {
            let ExpressionNode::Cast(cast) = self.program.expression_table.expression(expression)
            else {
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
                let ExpressionNode::Integer(literal) =
                    self.program.expression_table.expression(cast.value)
                else {
                    return None;
                };
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
    fn exact_computations(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        depth: usize,
    ) -> bool {
        use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
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

    fn current_expression(
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
            expression,
            bindings,
        })
    }

    fn current_bindings(
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

    fn current_value(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<super::super::prover::ScalarValue> {
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
        super::super::prover::evaluate_scalar(self.program, expression, &mut |leaf| {
            let subject = canonical_place_from_expression_in_state(
                self.program,
                self.exit.state_symbol,
                self.exit.statement_index,
                leaf,
            )?;
            super::super::prover::scalar_value_at_place(
                self.program,
                &self.facts.semantic,
                self.contexts
                    .iter()
                    .map(|context| self.facts.semantic.contexts.get(*context)),
                &subject,
            )
        })
    }
}

fn exact_integer_reference(
    program: &typed_trees::TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    program
        .primitive_type_reference(reference)
        .is_some_and(|primitive| primitive.accepts_integer_literal())
        && program.arithmetic_domain_for_type_reference(reference)
            == numerics::arithmetic::ArithmeticDomain::Exact
}

/// Traverse only the actual retained domain fact, preserving its declaration
/// owner for every operator query. The optional inner type is an anonymous
/// literal, not permission to borrow another operand's type. This traversal
/// establishes meaning and exact self occurrences; the shared evaluator and
/// implication engine, rather than this code, judge predicate truth.
fn domain_predicate_meaning(
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::domain::DomainDefinition,
    expression: typed_trees::expression::ExpressionHandle,
    subjects: &mut Vec<typed_trees::expression::ExpressionHandle>,
    depth: usize,
) -> Option<Option<typed_trees::types::TypeReferenceHandle>> {
    use language_core::OperatorSpelling;
    use typed_trees::expression::{BinaryOperator, ExpressionNode};
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let boolean_type = || {
        let symbol = program
            .symbols
            .child_handles(program.symbols.root())?
            .find(|symbol| {
                program.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::Bool)
            })?;
        program
            .type_reference_table
            .find_named_type_reference(symbol)
    };
    if let Some(reference) = validation::exact_domain_self_type(program, domain, expression) {
        if !subjects.contains(&expression) {
            subjects.push(expression);
        }
        return Some(Some(reference));
    }
    Some(match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => {
            validation::landed_integer_literal_type_reference(program, expression)
        }
        ExpressionNode::Boolean(_) => boolean_type(),
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
        {
            let parameter = typed_trees::domain::index_parameters(program, domain)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            let typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
            else {
                return None;
            };
            Some(type_reference)
        }
        ExpressionNode::Binary(binary) => {
            let left = domain_predicate_meaning(program, domain, binary.left, subjects, depth + 1)?;
            let right =
                domain_predicate_meaning(program, domain, binary.right, subjects, depth + 1)?;
            let spelling = match binary.operator {
                BinaryOperator::Equal => Some(OperatorSpelling::Equal),
                BinaryOperator::NotEqual => Some(OperatorSpelling::NotEqual),
                BinaryOperator::Less => Some(OperatorSpelling::Less),
                BinaryOperator::LessOrEqual => Some(OperatorSpelling::LessEqual),
                BinaryOperator::Greater => Some(OperatorSpelling::Greater),
                BinaryOperator::GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
                BinaryOperator::Add => Some(OperatorSpelling::Add),
                BinaryOperator::Subtract => Some(OperatorSpelling::Subtract),
                BinaryOperator::Multiply => Some(OperatorSpelling::Multiply),
                BinaryOperator::Divide => Some(OperatorSpelling::Divide),
                BinaryOperator::Modulo => Some(OperatorSpelling::Modulo),
                BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight => None,
                BinaryOperator::CaseMembership => return None,
            };
            if spelling.is_some_and(|spelling| {
                !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    domain.symbol,
                    expression,
                    spelling,
                    &[left, right],
                )
            }) {
                return None;
            }
            match binary.operator {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or => boolean_type(),
                _ => {
                    let exact_operand =
                        |operand, reference: Option<typed_trees::types::TypeReferenceHandle>| {
                            reference
                                .map(|reference| exact_integer_reference(program, reference))
                                .unwrap_or_else(|| {
                                    matches!(program.expression_table.expression(operand),
                                ExpressionNode::Integer(literal) if literal.landing().is_none())
                                })
                        };
                    if !exact_operand(binary.left, left) || !exact_operand(binary.right, right) {
                        return None;
                    }
                    let left_result = match left {
                        Some(reference) => Some(validation::arithmetic_result_type_reference(
                            program, reference,
                        )?),
                        None => None,
                    };
                    let right_result = match right {
                        Some(reference) => Some(validation::arithmetic_result_type_reference(
                            program, reference,
                        )?),
                        None => None,
                    };
                    left_result.or(right_result)
                }
            }
        }
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            domain_predicate_meaning(program, domain, unary.operand, subjects, depth + 1)?
        }
        _ => return None,
    })
}

/// Rebase an ensures fact rooted at the reserved `result` occurrence onto the
/// returned expression's canonical place, keeping the fact's own declared
/// field coordinates. `result.bytes` on a `rows[0]` return is exactly
/// `rows[0].bytes` at this statement -- the same place mutation invalidation
/// retires, so corrupted evidence cannot satisfy it.
pub(super) fn proves_result_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    contexts: &[FactContextHandle],
    requirement: &facts::Fact,
) -> bool {
    let domain_symbol = match requirement.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
        _ => return false,
    };
    let FactPlace::Place(place) = requirement.place else {
        return false;
    };
    let place = facts.semantic.places.get(place);
    let PlaceRoot::Expression(root) = place.root else {
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
    super::super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        contexts,
        &subject,
        domain_symbol,
    )
}

/// Every declared field predicate of an owned nominal (or fixed-array nominal
/// element) return type is an obligation on the returned value at each
/// value-returning exit -- the return-position dual of the declared-field
/// requirements a call's nominal input imposes on its actuals
/// (checks/contracts/nominal_inputs.rs). A readable reference return owns no
/// result storage but hands its caller the referent, so the same predicates
/// are obligations on the returned place: the caller reads the referent's
/// declared fields through the reference, and a source write that retired
/// them must have been repaired before the return.
pub(in crate::checks::contracts) fn check_result_field_domains(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == exit.machine_symbol)
    else {
        return;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    let mut paths = crate::facts::field_domain::declared_result_field_domain_paths(
        program,
        result_domain_type(program, entry.return_type),
    )
    .into_iter()
    .filter(|(_, domain_symbol)| value_provable_domain(program, *domain_symbol))
    .map(|(path, domain)| (path, domain, language_semantics::SemanticDomainId::NULL))
    .collect::<Vec<_>>();
    paths.extend(
        crate::facts::field_domain::declared_owned_field_domain_identities(
            program,
            result_domain_type(program, entry.return_type),
        ),
    );
    if program
        .primitive_type_reference(entry.return_type)
        .is_none()
    {
        paths.extend(
            crate::facts::field_domain::domain_constraint_identities(program, entry.return_type)
                .into_iter()
                .filter(|(domain, _)| {
                    crate::facts::field_domain::domain_requires_provenance(program, *domain)
                })
                .map(|(domain, identity)| (Vec::new(), domain, identity)),
        );
    }
    if paths.is_empty() {
        return;
    }
    let returned = exit_return_expression(program, exit);
    if !returned.is_valid() {
        return;
    }
    let entry_contexts: Vec<_> = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    let base = canonical_place_from_expression_in_state(
        program,
        exit.state_symbol,
        exit.statement_index,
        returned,
    );
    let cases =
        super::cases::CaseObservation::for_exit(program, facts, exit, &entry_contexts, call_frames);
    for (path, domain_symbol, semantic_domain) in paths {
        if cases
            .as_ref()
            .is_some_and(|cases| cases.result_path_is_inactive(&path))
        {
            continue;
        }
        let proves = |subject: &CanonicalPlace| {
            if exact_scalar_membership(
                program,
                facts,
                &entry_contexts,
                subject,
                domain_symbol,
                semantic_domain,
            ) {
                true
            } else if !crate::facts::field_domain::domain_requires_provenance(
                program,
                domain_symbol,
            ) {
                super::super::prover::prove_domain_at_place(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    subject,
                    domain_symbol,
                )
            } else {
                false
            }
        };
        // The same two shapes as call actuals (checks/contracts/nominal_inputs):
        // a returned constructor projects each requirement onto the exact field
        // or element expression it was built from; anything else proves at the
        // returned place directly.
        let satisfied = crate::flow::literal_value_projections(
            program,
            returned,
            entry.return_type,
            &path,
            false,
        )
        .is_some_and(|projections| {
            !projections.is_empty()
                && projections.iter().all(|projection| {
                    canonical_place_from_expression_in_state(
                        program,
                        exit.state_symbol,
                        exit.statement_index,
                        projection.expression,
                    )
                    .is_some_and(|mut subject| {
                        if !crate::flow::place_cases_are_selected(
                            program,
                            &facts.semantic,
                            &entry_contexts,
                            exit.machine_symbol,
                            exit.state_symbol,
                            exit.statement_index,
                            &subject,
                        ) {
                            return false;
                        }
                        subject.extend_segments(&projection.remaining);
                        proves(&subject)
                    })
                })
        }) || base.as_ref().is_some_and(|base| {
            if !crate::flow::place_cases_are_selected(
                program,
                &facts.semantic,
                &entry_contexts,
                exit.machine_symbol,
                exit.state_symbol,
                exit.statement_index,
                base,
            ) {
                return false;
            }
            let mut subject = base.clone();
            subject.extend_segments(&path);
            proves(&subject)
        });
        if !satisfied {
            let (root, mut segments) = base
                .as_ref()
                .map(|base| (base.root, base.segments.clone()))
                .unwrap_or((PlaceRoot::Expression(returned), Vec::new()));
            segments.extend_from_slice(&path);
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
                crate::labels::machine_name(program, exit.machine_symbol),
                exit.statement_index,
                crate::labels::canonical_place_label_from_parts(program, root, &segments),
                crate::labels::symbol_name(program, domain_symbol),
            )));
        }
    }
}

/// The type whose declared fields a return owes: the owned result type
/// itself, or the referent behind a readable reference return. A write-only
/// reference exposes no readable storage and owes nothing.
pub(crate) fn result_domain_type(
    program: &typed_trees::TypedTrees,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> typed_trees::types::TypeReferenceHandle {
    let mut reference = return_type;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => return *referee,
            _ => return reference,
        }
    }
    return_type
}

/// Whether a declared field domain is re-provable from the field's value.
/// An establishment-gated domain (`established by ..`) mints membership only
/// through its listed routes, so its rows are custody evidence transported
/// by the carry machinery rather than an invariant window a return can
/// close from predicates alone. Routed return obligations instead consume
/// exact live membership; predicate-only call reseeding must exclude them.
pub(crate) fn value_provable_domain(
    program: &typed_trees::TypedTrees,
    domain_symbol: symbols::SymbolHandle,
) -> bool {
    !crate::facts::field_domain::domain_requires_provenance(program, domain_symbol)
}

/// Whether a declared parameter type is a readable `&mut` reference, whose
/// referent the machine may corrupt and hands back at its return. Owned
/// parameters die with the machine and write-only views expose no readable
/// referent, so neither owes anything here.
pub(crate) fn is_readable_mutable_reference(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let mut reference = type_reference;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => {
                return *access == language_semantics::ReferenceAccess::Mutable;
            }
            _ => return false,
        }
    }
    false
}

/// The default-domain field facts a state assumes on entry for its readable
/// `&mut` referents: each such parameter's `StateParameterDomain` field facts
/// and, for `&mut self`, the machine's `MachineFieldDomain` facts. These are
/// exactly the rows the self-transition arrival check re-proves
/// (checks/contracts/arrivals.rs); no second obligation vocabulary exists.
pub(crate) fn mutable_referent_field_requirements(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
) -> Vec<facts::Fact> {
    let Some(state) =
        crate::semantic_calls::find_state_in_machine(program, machine_symbol, state_symbol)
    else {
        return Vec::new();
    };
    let mut referents = Vec::new();
    let mut self_is_mutable = false;
    for parameter in program.state_parameters(state) {
        if !is_readable_mutable_reference(program, parameter.type_reference) {
            continue;
        }
        if parameter.is_self {
            self_is_mutable = true;
        } else {
            referents.push(parameter.symbol);
        }
    }
    let mut requirements = Vec::new();
    if !referents.is_empty() {
        requirements.extend(
            semantic
                .contexts_at_point(ProgramPoint::State {
                    machine_symbol,
                    state_symbol,
                })
                .flat_map(|context| context.facts())
                .filter(|fact| {
                    matches!(
                        fact.origin,
                        FactOrigin::StateParameterDomain {
                            machine_symbol: origin_machine,
                            state_symbol: origin_state,
                        } if origin_machine == machine_symbol && origin_state == state_symbol
                    ) && matches!(fact.payload, FactPayload::DomainMembership { .. })
                        && matches!(fact.place, FactPlace::Place(place)
                            if matches!(semantic.places.get(place).root, PlaceRoot::Symbol(root)
                                if referents.contains(&root)))
                })
                .cloned(),
        );
    }
    if self_is_mutable {
        requirements.extend(
            semantic
                .contexts_at_point(ProgramPoint::Machine { machine_symbol })
                .flat_map(|context| context.facts())
                .filter(|fact| {
                    matches!(
                        fact.origin,
                        FactOrigin::MachineFieldDomain {
                            machine_symbol: origin_machine,
                        } if origin_machine == machine_symbol
                    ) && matches!(fact.payload, FactPayload::DomainMembership { domain_symbol, .. }
                        if value_provable_domain(program, domain_symbol))
                })
                .cloned(),
        );
    }
    requirements
}

/// A machine's normal return is a default-domain consumption point for every
/// readable `&mut` referent it received, `self` included: the field facts
/// assumed on entry must be provable again from the live evidence at the
/// exit. A write through a bare alias (`&mut [u8; 4]` onto a `Utf8` field)
/// retires the field's fact without establishing anything, so returning
/// without repairing it rejects here with the exact place, the same way a
/// call or transition would refuse the corrupted referent.
pub(in crate::checks::contracts) fn check_mutable_referent_field_domains(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let requirements = mutable_referent_field_requirements(
        program,
        &facts.semantic,
        exit.machine_symbol,
        exit.state_symbol,
    );
    let entry_contexts: Vec<_> = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    // Mutable roots use implicit signature preconditions rather than seeded
    // StateParameterDomain facts. Their declaration still creates a return
    // obligation, even when a write retired every copy of the entry evidence.
    if let Some(state) = crate::semantic_calls::find_state_in_machine(
        program,
        exit.machine_symbol,
        exit.state_symbol,
    ) {
        for parameter in program.state_parameters(state).iter().filter(|parameter| {
            !parameter.is_self && is_readable_mutable_reference(program, parameter.type_reference)
        }) {
            for (domain_symbol, semantic_domain) in
                crate::facts::field_domain::domain_constraint_identities(
                    program,
                    parameter.type_reference,
                )
                .into_iter()
                .filter(|(domain, _)| {
                    crate::facts::field_domain::domain_requires_provenance(program, *domain)
                })
            {
                let subject = CanonicalPlace {
                    root: PlaceRoot::Symbol(parameter.symbol),
                    segments: Vec::new(),
                };
                if !exact_scalar_membership(
                    program,
                    facts,
                    &entry_contexts,
                    &subject,
                    domain_symbol,
                    semantic_domain,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
                        crate::labels::machine_name(program, exit.machine_symbol), exit.statement_index,
                        parameter.name, crate::labels::symbol_name(program, domain_symbol),
                    )));
                }
            }
        }
    }
    for requirement in &requirements {
        let (
            FactPlace::Place(place),
            FactPayload::DomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            },
        ) = (requirement.place, requirement.payload)
        else {
            continue;
        };
        let place = facts.semantic.places.get(place);
        let satisfied =
            if crate::facts::field_domain::domain_requires_provenance(program, domain_symbol) {
                canonical_place_from_semantic_place(program, &facts.semantic, place).is_some_and(
                    |subject| {
                        exact_scalar_membership(
                            program,
                            facts,
                            &entry_contexts,
                            &subject,
                            domain_symbol,
                            semantic_domain,
                        )
                    },
                )
            } else {
                super::super::prover::semantic_contexts_prove_contract_fact(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    requirement,
                )
            };
        if satisfied {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
            crate::labels::machine_name(program, exit.machine_symbol),
            exit.statement_index,
            crate::labels::canonical_place_label_from_parts(
                program,
                place.root,
                facts.semantic.place_segments.span_or_empty(place.segments),
            ),
            crate::labels::symbol_name(program, domain_symbol),
        )));
    }
}
