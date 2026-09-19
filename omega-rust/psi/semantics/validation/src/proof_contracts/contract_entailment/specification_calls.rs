//! Selected call preconditions are obligations of contract term formation.
//!
//! Walk operands before judging their enclosing call. Only independently
//! formed, preceding requires facts are assumptions: neither the fact being
//! formed, another ensures, nor a future body citation can license a partial
//! application. Reflexive equality and body unfolding do not waive this gate.

use super::refuted_requires::instantiated_fact_judgment;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_terms::structural_term;
use crate::machine_calls::calls::machine_state_by_symbol;
use crate::value_custody::literals::expression_children::children;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::state::State;

pub(crate) fn validate_specification_call_requirements(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    fact: &ProofFact,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut roots = Vec::new();
    fact_roots(program, fact, &mut roots);
    let mut visited = Vec::new();
    for root in roots {
        validate_expression(
            program,
            machine,
            state,
            root,
            prior_facts,
            owner,
            diagnostics,
            &mut visited,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    visited: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    let node = program.expression_table.expression(expression);
    let diagnostics_before = diagnostics.len();
    children(program, node, |child| {
        validate_expression(
            program,
            machine,
            state,
            child,
            prior_facts,
            owner,
            diagnostics,
            visited,
        );
    });
    if diagnostics.len() != diagnostics_before {
        return;
    }
    let ExpressionNode::Call(call) = node else {
        return;
    };
    // Resolution, not target spelling, selects the declaration. Abstract
    // signature applications are not covered by this concrete-declaration query.
    let Some((callee, entry)) = selected_machine(program, call.target_symbol) else {
        return;
    };
    let mut requirements = applicable_requirements(program, callee, entry).peekable();
    if requirements.peek().is_none() {
        return;
    }
    if !requirements_are_acyclic(program, callee, entry, &mut Vec::new(), &mut Vec::new()) {
        diagnostics.push(Diagnostic::error(format!(
            "cyclic requires formation for specification call `{}` in {owner}: an application cannot establish the contract needed to form itself",
            callee.name,
        )));
        return;
    }
    let mut arguments = Vec::new();
    let parameters = program.state_parameters(entry);
    let static_receiver = call.receiver.is_valid()
        && matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path) if matches!(program.symbols.get(path.symbol).kind,
                SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::Domain
                | SymbolKind::Machine | SymbolKind::Module | SymbolKind::Trait
                | SymbolKind::ConformanceParameter)
        );
    if call.receiver.is_valid()
        && !static_receiver
        && parameters
            .first()
            .is_some_and(|parameter| parameter.is_self)
    {
        arguments.push(call.receiver);
    }
    arguments.extend_from_slice(program.expression_table.expression_handles(call.arguments));
    let substitution = parameters
        .iter()
        .zip(&arguments)
        .map(|(parameter, argument)| {
            Some((
                parameter.name.as_str().to_owned(),
                structural_term(program, *argument)?,
            ))
        })
        .collect::<Option<Vec<_>>>();
    // The existing structural prover has name-backed private variables. Do
    // not let two different resolved caller binders acquire one such variable.
    // The arithmetic route below uses exact symbols directly.
    let structural_facts = prior_facts
        .iter()
        .copied()
        .filter(|fact| {
            structurally_substitutable_fact(program, *fact)
                && names_are_unambiguous(program, &[*fact], &arguments)
        })
        .collect::<Vec<_>>();
    let structural_names_unique = names_are_unambiguous(program, &structural_facts, &arguments);
    let judge = StructuralJudge::from_requires(program, machine, &structural_facts);
    for requirement in requirements {
        let static_selection_is_concrete = call.machine_arguments.is_empty()
            && call.evidence_arguments.is_empty()
            && call.static_requirement_dispatch.is_none();
        let proven = if let ProofFact::Expression(required) = requirement
            && static_selection_is_concrete
        {
            let structural = requirement_parameters_are_bound(program, *required, entry)
                && structurally_substitutable_fact(program, *required)
                && parameters.len() == arguments.len()
                && structural_names_unique
                && substitution.as_ref().is_some_and(|substitution| {
                    substitution
                        .iter()
                        .all(|(_, term)| term_has_complete_substitution(program, term))
                        && matches!(
                            instantiated_fact_judgment(program, &judge, *required, substitution),
                            StructuralJudgment::Proven
                        )
                });
            structural
                || arithmetic_requirement(
                    program,
                    machine,
                    state,
                    entry,
                    prior_facts,
                    *required,
                    &arguments,
                )
        } else {
            false
        };
        if !proven {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove requires contract for specification call `{}` in {owner}: establish its selected precondition in an independently formed prior requires fact",
                callee.name,
            )));
        }
    }
}

fn applicable_requirements<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
) -> impl Iterator<Item = &'program ProofFact> {
    let is_entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    program
        .machine_contracts(machine)
        .iter()
        .filter(move |_| is_entry)
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
}

fn structurally_substitutable_fact(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return structurally_substitutable_fact(program, binary.left)
            && structurally_substitutable_fact(program, binary.right);
    }
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && [binary.left, binary.right].iter().all(|operand| {
        structural_term(program, *operand)
            .is_some_and(|term| term_has_complete_substitution(program, &term))
    })
}

/// Opaque display text is not a substitutable term. Applications are withheld
/// too: the legacy normalizer can turn their bodies into unsubstituted display
/// text even when the input application is structural. Neither a receiver
/// prefix rewrite nor equal-looking normalized text establishes exact operands.
/// Extend this judgment when that normalizer preserves argument and selected
/// callable identity; do not replace unknown with successful normalization.
fn term_has_complete_substitution(program: &TypedTrees, term: &StructuralTerm) -> bool {
    match term {
        StructuralTerm::Opaque(_)
        | StructuralTerm::Application { .. }
        | StructuralTerm::CallProjection { .. } => false,
        StructuralTerm::Variable(_) => true,
        StructuralTerm::Constructor { data, case, fields } => {
            constructor_fields_are_complete(program, data, case, fields)
                && fields
                    .iter()
                    .all(|(_, value)| term_has_complete_substitution(program, value))
        }
    }
}

fn constructor_fields_are_complete(
    program: &TypedTrees,
    data: &str,
    case: &str,
    fields: &[(String, StructuralTerm)],
) -> bool {
    use typed_trees::data::DataMember;
    if data == "bool" {
        return matches!(case, "true" | "false") && fields.is_empty();
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name.as_str() == data);
    let Some(definition) = definitions.next() else {
        return false;
    };
    if definitions.next().is_some() {
        return false;
    }
    let mut declared = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field.name.as_str()),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    if !case.is_empty() {
        let Some(variant) =
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case => Some(variant),
                    _ => None,
                })
        else {
            return false;
        };
        declared.extend(
            program
                .data_payload_fields(variant)
                .iter()
                .map(|field| field.name.as_str()),
        );
    }
    // Omitted fields denote ZII values, not absent comparison obligations.
    // Until the termifier materializes them, an incomplete field roster must
    // not reach the legacy constructor comparison (which scans one side).
    fields.len() == declared.len()
        && declared
            .iter()
            .all(|name| fields.iter().filter(|(field, _)| field == name).count() == 1)
}

fn fact_roots(program: &TypedTrees, fact: &ProofFact, roots: &mut Vec<ExpressionHandle>) {
    match fact {
        ProofFact::Expression(expression) => roots.push(*expression),
        ProofFact::Membership(membership) => roots.push(membership.value),
        ProofFact::Proposition(application) => roots.extend_from_slice(
            program
                .expression_table
                .expression_handles(application.arguments),
        ),
    }
}

/// Declaration validation also checks every requirement's formation, but it
/// cannot justify a cycle of mutually dependent contracts. Check just those
/// dependencies, not body recursion (whose well-foundedness has another owner).
fn requirements_are_acyclic(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    active: &mut Vec<SymbolHandle>,
    complete: &mut Vec<SymbolHandle>,
) -> bool {
    if complete.contains(&state.symbol) {
        return true;
    }
    if active.contains(&state.symbol) {
        return false;
    }
    active.push(state.symbol);
    let mut pending = Vec::new();
    for fact in applicable_requirements(program, machine, state) {
        fact_roots(program, fact, &mut pending);
    }
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Call(call) = node
            && let Some((callee, target)) = selected_machine(program, call.target_symbol)
            && !requirements_are_acyclic(program, callee, target, active, complete)
        {
            return false;
        }
        children(program, node, |child| pending.push(child));
    }
    active.pop();
    complete.push(state.symbol);
    true
}

fn requirement_parameters_are_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
    state: &State,
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(path) = node {
            for symbol in [path.head_symbol, path.symbol] {
                if program.symbols.get(symbol).kind == SymbolKind::Parameter
                    && !program
                        .state_parameters(state)
                        .iter()
                        .any(|parameter| parameter.symbol == symbol)
                {
                    return false;
                }
            }
        }
        children(program, node, |child| pending.push(child));
    }
    true
}

fn selected_machine(program: &TypedTrees, target: SymbolHandle) -> Option<(&Machine, &State)> {
    machine_state_by_symbol(program, target).or_else(|| {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == target && target.is_valid())?;
        Some((machine, program.machine_states(machine).first()?))
    })
}

fn arithmetic_requirement(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    callee: &State,
    prior_facts: &[ExpressionHandle],
    requirement: ExpressionHandle,
    arguments: &[ExpressionHandle],
) -> bool {
    use super::{
        ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
        ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
        scoped_arithmetic_implication,
    };
    let parameters = program.state_parameters(callee);
    if parameters.len() != arguments.len() {
        return false;
    }
    let mut bindings = Vec::new();
    for caller_state in program
        .machine_states(machine)
        .first()
        .into_iter()
        .chain(state)
    {
        for parameter in program.state_parameters(caller_state) {
            let Some(primitive) = crate::value_custody::recasts::exact_primitive_type(
                program,
                parameter.type_reference,
            ) else {
                continue;
            };
            if !primitive.accepts_integer_literal()
                || !parameter.symbol.is_valid()
                || bindings.iter().any(|binding: &ScopedArithmeticBinding| {
                    binding.binder == ScopedArithmeticBinder::Symbol(parameter.symbol)
                })
            {
                continue;
            }
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Atom {
                    identity: format!("\0specification:{:?}", parameter.symbol),
                    unsigned: !primitive.is_signed_integer(),
                },
            });
        }
    }
    let hypotheses = prior_facts
        .iter()
        .map(|expression| ScopedArithmeticHypothesis {
            proposition: ScopedArithmeticExpression {
                expression: *expression,
                bindings: bindings.clone(),
            },
            holds: true,
        })
        .collect::<Vec<_>>();
    // Separate rosters keep self-call argument substitution from rewriting
    // the caller's hypotheses, even when caller and callee symbols coincide.
    let goal = ScopedArithmeticExpression {
        expression: requirement,
        bindings: parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                    expression: *argument,
                    bindings: bindings.clone(),
                }),
            })
            .collect(),
    };
    matches!(
        scoped_arithmetic_implication(program, machine, &hypotheses, &goal),
        StrictArithmeticImplicationJudgment::Proven
    )
}

fn names_are_unambiguous(
    program: &TypedTrees,
    hypotheses: &[ExpressionHandle],
    arguments: &[ExpressionHandle],
) -> bool {
    let mut pending = hypotheses
        .iter()
        .chain(arguments)
        .copied()
        .collect::<Vec<_>>();
    let mut visited = Vec::new();
    let mut names: Vec<(String, SymbolHandle)> = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(name) = node {
            if let Some(head) = program
                .expression_table
                .name_path_members(name.members)
                .first()
            {
                if let Some((_, symbol)) = names.iter().find(|(known, _)| known == head.as_str()) {
                    if *symbol != name.head_symbol {
                        return false;
                    }
                } else {
                    names.push((head.as_str().to_owned(), name.head_symbol));
                }
            }
            let spelling = program.expression_table.display_name(expression);
            if let Some((_, symbol)) = names.iter().find(|(known, _)| *known == spelling) {
                if *symbol != name.symbol {
                    return false;
                }
            } else {
                names.push((spelling, name.symbol));
            }
        }
        children(program, node, |child| pending.push(child));
    }
    true
}
