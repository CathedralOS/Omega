//! Use-site discharge for `requires` contracts carried by the SELECTED meaning
//! of a spelled binary operator or implicit Match equality.
//!
//! Model: the bounds-from-`requires` seam for `[]`/`[..]`
//! (checks/ranges/indexes/validation.rs) sources the access obligation from
//! the spelled operator's `requires` contract and discharges it with the
//! bounds prover. Spelled binary uses have no special-purpose prover, so the
//! obligation is INSTANTIATED over the actual operands (parameter -> operand,
//! the call-`requires` instantiation precedent in
//! checks/contracts/labels/calls.rs) and proven against the semantic contexts
//! at the exact invocation after operand effects, for both spelled operators
//! and implicit Match comparisons. Statement-entry facts can describe overwritten
//! storage or omit guarantees established by earlier operands. Copied scalars
//! retain operand-time facts; other carriers additionally require those same
//! facts to remain live at invocation. This conservative intersection prevents
//! a later guarantee or reference rebind from impersonating a captured value.
//! This query checks the already
//! selected operator's precondition; it never participates in selecting the
//! operator meaning itself.
//!
//! An unproven obligation reports the operator-contract attribution shape the
//! indexed seam established: name the instantiated clause, the operator that
//! declares it, and the spelling that resolved to it, so the user can browse
//! to the operator declaration and read the governing contract.

use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use facts::{FactContextHandle, FactPayload, FactPlace, FactPlan};
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::OperatorDefinition;
use typed_trees::signature::{SignatureContractKind, StateParameter};

use super::super::contracts::labels::domain_proves_expression_label;
use crate::labels::{
    canonical_place_label, instantiate_operator_contract_expression_label,
    semantic_boolean_fact_label, symbol_name,
};

mod invocation;
use invocation::InvocationContexts;

/// Crash refinement needs proof of falsity, not failure to prove a precondition.
/// Only Boolean structure and exact evaluated predicate facts supply polarity;
/// selected comparisons have no inferred arithmetic/complement laws here.
pub(crate) fn operator_route_is_false(
    program: &TypedTrees,
    flow: &checked_trees::FlowFacts,
    semantic: &FactPlan,
    operator_use: arena::Handle<checked_trees::CheckedOperatorUseFact>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let contexts = InvocationContexts::from_flow(flow, operator_use, operands);
    expression_has_polarity(
        program, semantic, &contexts, parameters, operands, expression, false,
    )
}

fn expression_has_polarity(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
    polarity: bool,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => return *value == polarity,
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            return expression_has_polarity(
                program,
                semantic,
                contexts,
                parameters,
                operands,
                unary.operand,
                !polarity,
            );
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            let left = expression_has_polarity(
                program,
                semantic,
                contexts,
                parameters,
                operands,
                binary.left,
                polarity,
            );
            let right = expression_has_polarity(
                program,
                semantic,
                contexts,
                parameters,
                operands,
                binary.right,
                polarity,
            );
            return if (binary.operator == BinaryOperator::And) == polarity {
                left && right
            } else {
                left || right
            };
        }
        _ => {}
    }
    if polarity {
        return contexts_prove_boolean_leaf(
            program, semantic, contexts, parameters, operands, expression,
        );
    }
    let required =
        instantiate_operator_contract_expression_label(program, parameters, operands, expression);
    contexts
        .for_expressions(program, parameters, [expression])
        .iter()
        .any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    matches!(fact.payload, FactPayload::BooleanValue { expression, value: false }
                if program.expression_table.display_name(expression) == required)
                })
        })
}

/// Checks selected binary and implicit comparison preconditions against their
/// available invocation facts, reporting each unproven clause.
/// Slice `[]`/`[..]` uses discharge through the ranges seam and are
/// deliberately excluded.
pub(super) fn selected_binary_requires_diagnostics(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (operator_use_handle, operator_use) in facts.operators.uses.iter() {
        if operator_use.status != checked_trees::CheckedOperatorResolutionStatus::Resolved {
            continue;
        }
        if let checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm } =
            operator_use.occurrence
            && let ExpressionNode::Match(dispatch) =
                program.expression_table.expression(operator_use.expression)
            && let Some(ordinal) = source_arm
                .arena_index()
                .checked_sub(dispatch.arms.start().arena_index())
            && program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .take(ordinal as usize)
                .any(|arm| matches!(arm.pattern, typed_trees::expression::MatchPattern::Wildcard))
        {
            continue;
        }
        if matches!(
            operator_use.spelling,
            OperatorSpelling::Index | OperatorSpelling::Range
        ) {
            continue;
        }
        let Some(selected) = facts.operators.selected_candidate(operator_use) else {
            continue;
        };
        let requires_facts: Vec<&ProofFact> = program
            .signature_contracts
            .span_or_empty(selected.contracts)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .collect();
        if requires_facts.is_empty() {
            continue;
        }

        let operator = operator_definition(program, selected.operator_symbol);
        let parameters = operator
            .map(|operator| program.operator_parameters(operator))
            .unwrap_or(&[]);
        let operands = operator_use.operands(program).unwrap_or_default();
        let invocation_contexts = InvocationContexts::new(facts, operator_use_handle, &operands);

        for fact in requires_facts {
            let proven = requires_fact_proven(
                program,
                &facts.semantic,
                &invocation_contexts,
                parameters,
                &operands,
                fact,
            );
            if !proven {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove `{}` — the `requires` of `{}` (spelled `{}`)",
                    requires_clause_label(program, parameters, &operands, fact),
                    operator_path_label(
                        program,
                        operator,
                        selected.domain_symbol,
                        selected.operator_symbol
                    ),
                    operator_use.spelling.symbol(),
                )));
            }
        }
    }

    diagnostics
}

/// The selected operator's definition; candidates record only the symbol, so
/// resolve it back to the declaration (root surface or domain-owned).
fn operator_definition(
    program: &TypedTrees,
    operator_symbol: SymbolHandle,
) -> Option<&OperatorDefinition> {
    program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain).iter()),
        )
        .find(|operator| operator.symbol == operator_symbol)
}

/// The browsable path naming the operator in a failed-contract attribution,
/// e.g. `Quantity::Additive::add` for a domain-owned meaning or `Slice::index`
/// for a root surface declaration.
fn operator_path_label(
    program: &TypedTrees,
    operator: Option<&OperatorDefinition>,
    domain_symbol: SymbolHandle,
    operator_symbol: SymbolHandle,
) -> String {
    let path = operator
        .map(|operator| {
            program
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect::<Vec<_>>()
                .join("::")
        })
        .unwrap_or_default();
    let path = if path.is_empty() {
        symbol_name(program, operator_symbol)
    } else {
        path
    };
    if domain_symbol.is_valid() {
        format!("{}::{path}", symbol_name(program, domain_symbol))
    } else {
        path
    }
}

/// Whether one instantiated `requires` fact is proven by any context entering
/// the invocation.
fn requires_fact_proven(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    fact: &ProofFact,
) -> bool {
    match fact {
        ProofFact::Membership(membership) => {
            if !membership.domain_arguments.is_empty() {
                return false;
            }
            let value_label = instantiate_operator_contract_expression_label(
                program,
                parameters,
                operands,
                membership.value,
            );
            let contexts = contexts.for_expressions(program, parameters, [membership.value]);
            contexts.iter().any(|context| {
                context_proves_membership_label(
                    program,
                    semantic,
                    *context,
                    &value_label,
                    membership.domain_symbol,
                )
            })
        }
        ProofFact::Expression(expression) => contexts_prove_boolean_expression(
            program,
            semantic,
            contexts,
            parameters,
            operands,
            *expression,
        ),
        ProofFact::Proposition(application) => {
            let contexts = contexts.for_expressions(
                program,
                parameters,
                program
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .copied(),
            );
            let binder_labels = application
                .binder_arguments
                .iter()
                .map(|argument| argument.display_name())
                .collect::<Vec<_>>();
            let argument_labels = program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| {
                    instantiate_operator_contract_expression_label(
                        program, parameters, operands, *argument,
                    )
                })
                .collect::<Vec<_>>();
            let Some(required_label) = program
                .normalize_proposition_application_with_labels(
                    application,
                    &binder_labels,
                    &argument_labels,
                )
                .map(|formula| formula.identity_label())
            else {
                return false;
            };
            contexts.iter().any(|context| {
                let context = semantic.contexts.get(*context);
                semantic
                    .context_view(context)
                    .proves_proposition_label(program, &required_label)
                    || required_label
                        .strip_prefix("boolean:")
                        .is_some_and(|required_boolean| {
                            semantic.context_view(context).facts().any(|candidate| {
                                semantic
                                    .boolean_fact_label(program, candidate)
                                    .is_some_and(|label| label == required_boolean)
                            })
                        })
            })
        }
    }
}

/// Boolean `requires` clauses decompose like the call-`requires` prover:
/// conjunctions need both sides, disjunctions either side, and a leaf is
/// proven by a matching instantiated fact (directly, or derived from a domain
/// membership whose body states the clause).
fn contexts_prove_boolean_expression(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => contexts_prove_boolean_expression(
            program,
            semantic,
            contexts,
            parameters,
            operands,
            inner.target,
        ),
        ExpressionNode::Boolean(true) => true,
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::And => {
                contexts_prove_boolean_expression(
                    program,
                    semantic,
                    contexts,
                    parameters,
                    operands,
                    binary.left,
                ) && contexts_prove_boolean_expression(
                    program,
                    semantic,
                    contexts,
                    parameters,
                    operands,
                    binary.right,
                )
            }
            BinaryOperator::Or => {
                contexts_prove_boolean_expression(
                    program,
                    semantic,
                    contexts,
                    parameters,
                    operands,
                    binary.left,
                ) || contexts_prove_boolean_expression(
                    program,
                    semantic,
                    contexts,
                    parameters,
                    operands,
                    binary.right,
                )
            }
            _ => contexts_prove_boolean_leaf(
                program, semantic, contexts, parameters, operands, expression,
            ),
        },
        _ => contexts_prove_boolean_leaf(
            program, semantic, contexts, parameters, operands, expression,
        ),
    }
}

fn contexts_prove_boolean_leaf(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let required_label =
        instantiate_operator_contract_expression_label(program, parameters, operands, expression);
    let contexts = contexts.for_expressions(program, parameters, [expression]);
    contexts
        .iter()
        .any(|context| context_proves_boolean_label(program, semantic, *context, &required_label))
}

/// A leaf clause is proven by a boolean fact whose rendered form matches the
/// instantiated clause, or by a domain membership whose `requires` states it
/// over the member value (mirroring the call-`requires` leaf prover in
/// checks/contracts/direct.rs and domains.rs).
fn context_proves_boolean_label(
    program: &TypedTrees,
    semantic: &FactPlan,
    context: FactContextHandle,
    required_label: &str,
) -> bool {
    let context = semantic.contexts.get(context);
    semantic
        .context_view(context)
        .facts()
        .any(|fact| match fact.payload {
            // A reached true branch establishes this evaluated predicate, not
            // a mathematical interpretation of its selected operator spelling.
            FactPayload::BooleanValue {
                expression,
                value: true,
            }
            | FactPayload::BooleanExpression(expression) => {
                true_expression_proves_label(program, expression, required_label)
            }
            FactPayload::ContractBooleanExpression {
                expression,
                instantiated,
                ..
            } => {
                // Declaration-shaped facts retain their actual expression.
                // An instantiated fact instead names caller operands: never
                // reuse its unsubstituted formal expression as caller evidence.
                (!instantiated.is_valid()
                    && true_expression_proves_label(program, expression, required_label))
                    || semantic_boolean_fact_label(program, semantic, fact)
                        .is_some_and(|candidate| candidate == required_label)
            }
            FactPayload::DomainMembership {
                domain_symbol,
                value,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain_symbol,
                value,
                ..
            } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                let canonical_base =
                    canonical_place_label(program, semantic, semantic.places.get(place));
                let display_base = program.expression_table.display_name(value);
                domain_proves_expression_label(
                    program,
                    domain_symbol,
                    &canonical_base,
                    required_label,
                ) || domain_proves_expression_label(
                    program,
                    domain_symbol,
                    &display_base,
                    required_label,
                )
            }
            _ => false,
        })
}

fn true_expression_proves_label(
    program: &TypedTrees,
    expression: ExpressionHandle,
    required_label: &str,
) -> bool {
    if program.expression_table.display_name(expression) == required_label {
        return true;
    }
    // A retained true conjunction supplies each conjunct. Do not derive an
    // opposite comparison from a false observation: selected comparisons need
    // not share builtin arithmetic or equality laws.
    matches!(program.expression_table.expression(expression),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And
            && (true_expression_proves_label(program, binary.left, required_label)
                || true_expression_proves_label(program, binary.right, required_label)))
}

/// An instantiated membership obligation (`<operand> in Domain`) is proven by
/// a context fact placing the same value in a domain that implies the required
/// one.
fn context_proves_membership_label(
    program: &TypedTrees,
    semantic: &FactPlan,
    context: FactContextHandle,
    value_label: &str,
    required_domain: SymbolHandle,
) -> bool {
    if !typed_trees::domain::supports_symbol_only_proof(program, required_domain) {
        return false;
    }
    let context = semantic.contexts.get(context);
    semantic.context_view(context).facts().any(|fact| {
        let (fact_domain, fact_value) = match fact.payload {
            FactPayload::DomainMembership {
                domain_symbol,
                value,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain_symbol,
                value,
                ..
            } => (domain_symbol, value),
            _ => return false,
        };
        if !typed_trees::domain::supports_symbol_only_proof(program, fact_domain) {
            return false;
        }
        if !semantic.domain_implies(fact_domain, required_domain)
            && !crate::field_domain::domain_membership_implies(
                program,
                fact_domain,
                required_domain,
            )
        {
            return false;
        }
        let place_matches = matches!(fact.place, FactPlace::Place(place)
            if canonical_place_label(program, semantic, semantic.places.get(place)) == value_label);
        place_matches || program.expression_table.display_name(fact_value) == value_label
    })
}

/// The unproven clause in caller terms for the attribution diagnostic.
fn requires_clause_label(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    fact: &ProofFact,
) -> String {
    match fact {
        ProofFact::Expression(expression) => instantiate_operator_contract_expression_label(
            program,
            parameters,
            operands,
            *expression,
        ),
        ProofFact::Membership(membership) => format!(
            "{} in {}",
            instantiate_operator_contract_expression_label(
                program,
                parameters,
                operands,
                membership.value,
            ),
            symbol_name(program, membership.domain_symbol)
        ),
        ProofFact::Proposition(application) => format!(
            "{}({})",
            application.name.as_str(),
            program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| instantiate_operator_contract_expression_label(
                    program, parameters, operands, *argument,
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
