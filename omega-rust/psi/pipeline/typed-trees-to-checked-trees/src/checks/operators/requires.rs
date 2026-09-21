//! Use-site discharge for `requires` contracts carried by the SELECTED meaning
//! of a spelled binary operator, an implicit Match equality, or a named
//! `Namespace::requirement(...)` operator call.
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
//! and by-value carriers with stable observable contents retain operand-time
//! facts — the bound operand is a detached copy a later write cannot reach;
//! view carriers such as references and slices additionally require those same
//! facts to remain live at invocation. This conservative intersection prevents
//! a later guarantee or reference rebind from impersonating a captured value.
//! This query checks the already
//! selected operator's precondition; it never participates in selecting the
//! operator meaning itself.
//!
//! Named operator calls carry the same obligation. They produce no `uses`
//! row, so their operand-time capture keys on the `named_use` handle and
//! their operand list is reconstructed in operator-parameter order by
//! `named_call_operands` — including an `is_self` or leading value receiver
//! and skipping a static namespace receiver, which is a path, not an operand.
//! A reference formal still reads its referent in the predicate, so operand
//! labels come from `named_call_operand_labels`. A call whose evaluation
//! emitted no capture row, or whose captured operand expressions do not match
//! exactly, selects no contexts — the same fail-closed shape a spelled use
//! gets for missing, duplicated, or substituted custody.
//!
//! An unproven obligation reports the operator-contract attribution shape the
//! indexed seam established: name the instantiated clause, the operator that
//! declares it, and the spelling that resolved to it, so the user can browse
//! to the operator declaration and read the governing contract.

use std::cmp::Ordering;

use checked_trees::{CheckFacts, CheckedOperatorFacts, CheckedOperatorResolutionStatus};
use diagnostics::Diagnostic;
use facts::{FactContextHandle, FactPayload, FactPlace, FactPlan};
use language_core::operator_spelling::OperatorSpelling;
use numerics::bignum::{BigInt, BigRational, ExactFloat, IeeeRounding};
use numerics::literals::{FloatFormat, FloatLiteral};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::operator::OperatorDefinition;
use typed_trees::proposition::PropositionLabels;
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::types::PrimitiveType;

use super::super::contracts::labels::domain_proves_expression_label;
use crate::labels::{
    canonical_place_label, instantiate_operator_contract_expression_label_with_labels,
    semantic_boolean_fact_label, symbol_name,
};

mod invocation;
use invocation::InvocationContexts;

#[cfg(test)]
mod tests;

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
        program,
        semantic,
        &contexts,
        parameters,
        &operand_labels(program, operands),
        expression,
        false,
    )
}

/// The same discharge for a named `Namespace::requirement(...)` call whose
/// evaluation emitted an operand-time capture row keyed by its `named_use`
/// handle. The selection rules are identical to the spelled path: operand
/// expressions must match exactly, scalar carriers use their operand-time
/// snapshots, and other carriers intersect those snapshots with facts live
/// at invocation. The caller supplies the operand labels: a reference formal
/// used as a predicate value reads its referent, so a `&x` operand names
/// `x`. A named use whose statement produced no capture has no row here at
/// all; the caller applies its entry-context fallback instead.
#[allow(clippy::too_many_arguments)]
pub(crate) fn named_operator_route_is_false(
    program: &TypedTrees,
    flow: &checked_trees::FlowFacts,
    semantic: &FactPlan,
    named_use: arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    operand_labels: &[String],
    expression: ExpressionHandle,
) -> bool {
    let contexts = InvocationContexts::from_named_use(flow, named_use, operands);
    expression_has_polarity(
        program,
        semantic,
        &contexts,
        parameters,
        operand_labels,
        expression,
        false,
    )
}

/// Operand labels as the spelled surface renders them. The named-call seam
/// supplies its own referent-naming labels instead.
fn operand_labels(program: &TypedTrees, operands: &[ExpressionHandle]) -> Vec<String> {
    operands
        .iter()
        .map(|operand| {
            program.render_proof_expression(
                *operand,
                typed_trees::proposition::ProofSubstitutions::None,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn expression_has_polarity(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operand_labels: &[String],
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
                operand_labels,
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
                operand_labels,
                binary.left,
                polarity,
            );
            let right = expression_has_polarity(
                program,
                semantic,
                contexts,
                parameters,
                operand_labels,
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
            program,
            semantic,
            contexts,
            parameters,
            operand_labels,
            expression,
        );
    }
    let required = instantiate_operator_contract_expression_label_with_labels(
        program,
        parameters,
        operand_labels,
        expression,
    );
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

/// Checks selected binary, implicit comparison, and named-call preconditions
/// against their available invocation facts, reporting each unproven clause.
/// Slice `[]`/`[..]` uses discharge through the ranges seam and are
/// deliberately excluded — including a named call whose selected operator
/// carries that spelling.
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
        let operand_labels = operand_labels(program, &operands);
        let invocation_contexts = InvocationContexts::new(facts, operator_use_handle, &operands);

        for fact in requires_facts {
            let proven = requires_fact_proven(
                program,
                &facts.semantic,
                &facts.operators,
                &invocation_contexts,
                parameters,
                &operands,
                &operand_labels,
                fact,
            );
            if !proven {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove `{}` — the `requires` of `{}` (spelled `{}`)",
                    requires_clause_label(program, parameters, &operand_labels, fact),
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

    // Named `Namespace::requirement(...)` calls carry the same selected
    // `requires` obligations as spelled uses. Their operand-time capture keys
    // on the `named_use` handle, and `named_call_operands` reconstructs the
    // operand list in operator-parameter order — an `is_self` or leading
    // value receiver is an operand while a static namespace receiver is only
    // a path. A reference formal reads its referent in the predicate, so the
    // operand labels name referents rather than borrow expressions.
    for (named_use_handle, named_use) in facts.operators.named_uses.iter() {
        let Some(operator) = typed_trees::operator::declaration_by_symbol(
            program,
            named_use.selected_operator_symbol,
        ) else {
            continue;
        };
        // The ranges seam owns `[]`/`[..]` discharge for the spelling it
        // recognizes; a named call to such an operator keeps that split.
        if matches!(
            operator.spelling,
            Some(OperatorSpelling::Index | OperatorSpelling::Range)
        ) {
            continue;
        }
        let requires_facts: Vec<&ProofFact> = program
            .signature_contracts
            .span_or_empty(operator.contracts)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .collect();
        if requires_facts.is_empty() {
            continue;
        }
        let invalid = || {
            Diagnostic::error(
                "selected named operator call requires exact selected meaning and ordered operand capture",
            )
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(named_use.expression)
        else {
            diagnostics.push(invalid());
            continue;
        };
        let parameters = program.operator_parameters(operator);
        let Some(operands) =
            crate::facts::operator_crashes::named_call_operands(program, call, parameters)
        else {
            diagnostics.push(invalid());
            continue;
        };
        let operand_labels = crate::facts::operator_crashes::named_call_operand_labels(
            program, parameters, &operands,
        );
        let invocation_contexts =
            InvocationContexts::from_named_use(&facts.flow, named_use_handle, &operands);

        for fact in requires_facts {
            let proven = requires_fact_proven(
                program,
                &facts.semantic,
                &facts.operators,
                &invocation_contexts,
                parameters,
                &operands,
                &operand_labels,
                fact,
            );
            if !proven {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove `{}` — the `requires` of `{}` (called `{}`)",
                    requires_clause_label(program, parameters, &operand_labels, fact),
                    operator_path_label(
                        program,
                        Some(operator),
                        operator.home_domain,
                        operator.symbol
                    ),
                    program.expression_table.display_name(named_use.expression),
                )));
            }
        }
    }

    // The same residual species for `requires`: a `StatementNode::Call` that
    // still resolves to a requires-bearing operator minted no `named_uses`
    // row — only return-typed selections are rewritten into the `LocalData`
    // expression form operand capture sees. A survivor (a declaration with
    // no result type, or a requirement the rewrite does not select) must not
    // pass with its preconditions unexamined.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
            {
                let typed_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                let Some(operator) = crate::flow::resolved_operator_statement_symbol(program, call)
                    .and_then(|symbol| {
                        typed_trees::operator::declaration_by_symbol(program, symbol)
                    })
                else {
                    continue;
                };
                if !program
                    .signature_contracts
                    .span_or_empty(operator.contracts)
                    .iter()
                    .any(|contract| contract.kind == SignatureContractKind::Requires)
                {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "statement call to named operator `{}` carries `requires` obligations no checked capture can prove",
                    operator_path_label(
                        program,
                        Some(operator),
                        operator.home_domain,
                        operator.symbol
                    ),
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
/// the invocation, or is a closed-literal claim decidable on its own.
fn requires_fact_proven(
    program: &TypedTrees,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    operand_labels: &[String],
    fact: &ProofFact,
) -> bool {
    match fact {
        ProofFact::Membership(membership) => {
            if !membership.domain_arguments.is_empty() {
                return false;
            }
            let value_label = instantiate_operator_contract_expression_label_with_labels(
                program,
                parameters,
                operand_labels,
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
            operators,
            contexts,
            parameters,
            operands,
            operand_labels,
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
                    instantiate_operator_contract_expression_label_with_labels(
                        program,
                        parameters,
                        operand_labels,
                        *argument,
                    )
                })
                .collect::<Vec<_>>();
            let Some(required_label) = program
                .normalize_proposition_application(
                    application,
                    Some(PropositionLabels {
                        binder_labels: &binder_labels,
                        argument_labels: &argument_labels,
                    }),
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
/// membership whose body states the clause) or is a closed-literal claim.
fn contexts_prove_boolean_expression(
    program: &TypedTrees,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    operand_labels: &[String],
    expression: ExpressionHandle,
) -> bool {
    let leaf_is_proven = |expression| {
        contexts_prove_boolean_leaf(
            program,
            semantic,
            contexts,
            parameters,
            operand_labels,
            expression,
        ) || instantiated_leaf_is_closed_true(program, operators, parameters, operands, expression)
            || contexts_prove_case_membership(
                program, semantic, contexts, parameters, operands, expression,
            )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => contexts_prove_boolean_expression(
            program,
            semantic,
            operators,
            contexts,
            parameters,
            operands,
            operand_labels,
            inner.target,
        ),
        ExpressionNode::Boolean(true) => true,
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::And => {
                contexts_prove_boolean_expression(
                    program,
                    semantic,
                    operators,
                    contexts,
                    parameters,
                    operands,
                    operand_labels,
                    binary.left,
                ) && contexts_prove_boolean_expression(
                    program,
                    semantic,
                    operators,
                    contexts,
                    parameters,
                    operands,
                    operand_labels,
                    binary.right,
                )
            }
            BinaryOperator::Or => {
                contexts_prove_boolean_expression(
                    program,
                    semantic,
                    operators,
                    contexts,
                    parameters,
                    operands,
                    operand_labels,
                    binary.left,
                ) || contexts_prove_boolean_expression(
                    program,
                    semantic,
                    operators,
                    contexts,
                    parameters,
                    operands,
                    operand_labels,
                    binary.right,
                )
            }
            _ => leaf_is_proven(expression),
        },
        _ => leaf_is_proven(expression),
    }
}

fn contexts_prove_boolean_leaf(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operand_labels: &[String],
    expression: ExpressionHandle,
) -> bool {
    let required_label = instantiate_operator_contract_expression_label_with_labels(
        program,
        parameters,
        operand_labels,
        expression,
    );
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
            && !crate::facts::field_domain::domain_membership_implies(
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

/// The unproven clause in caller terms for the attribution diagnostic. Both
/// use kinds pass their own operand labels: a spelled use renders operands as
/// written, while a named call names the referent under a reference formal.
fn requires_clause_label(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operand_labels: &[String],
    fact: &ProofFact,
) -> String {
    match fact {
        ProofFact::Expression(expression) => {
            instantiate_operator_contract_expression_label_with_labels(
                program,
                parameters,
                operand_labels,
                *expression,
            )
        }
        ProofFact::Membership(membership) => format!(
            "{} in {}",
            instantiate_operator_contract_expression_label_with_labels(
                program,
                parameters,
                operand_labels,
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
                .map(|argument| {
                    instantiate_operator_contract_expression_label_with_labels(
                        program,
                        parameters,
                        operand_labels,
                        *argument,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// A scalar atom whose value is fixed by the source text alone. A float
/// literal keeps its authored landing; the comparison site picks the common
/// format from a substituted formal's declared type when one is available.
enum ClosedScalar {
    Integer(BigInt),
    Float(FloatLiteral),
    Boolean(bool),
}

/// An instantiated `requires` leaf whose atoms are all closed literals is a
/// concrete claim: `70 == 70` needs no context fact because the operand
/// written at the call is its own evidence. The same holds for the float
/// conversion contracts — `value == value && value > -129.0 && value < 128.0`
/// on a `-8.75f32` operand is an exact finite-range check, decided here at the
/// literal's own format. This grants no operator law: the leaf evaluates only
/// when every operator inside it resolved to a builtin meaning, so a
/// comparison that selected a checked or boundary-operator meaning keeps the
/// label/fact path. The channel is context-free — operand custody is about
/// which facts describe the operands, and a literal atom carries no premise.
fn instantiated_leaf_is_closed_true(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    closed_clause_value(program, parameters, operands, expression) == Some(true)
        && closed_operators_are_builtin(program, operators, expression)
}

/// Evaluate a contract-side Boolean leaf (or its Boolean structure) to a
/// closed value. Formal `Name` leaves substitute their operand expression;
/// anything not reducible to literals yields `None`.
fn closed_clause_value(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<bool> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(*value),
        ExpressionNode::Borrow(borrow) => {
            closed_clause_value(program, parameters, operands, borrow.target)
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            closed_clause_value(program, parameters, operands, unary.operand).map(|value| !value)
        }
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::And => {
                let left = closed_clause_value(program, parameters, operands, binary.left)?;
                let right = closed_clause_value(program, parameters, operands, binary.right)?;
                Some(left && right)
            }
            BinaryOperator::Or => {
                let left = closed_clause_value(program, parameters, operands, binary.left)?;
                let right = closed_clause_value(program, parameters, operands, binary.right)?;
                Some(left || right)
            }
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => {
                let left = closed_scalar_value(program, parameters, operands, binary.left)?;
                let right = closed_scalar_value(program, parameters, operands, binary.right)?;
                let format = clause_comparison_format(
                    program,
                    parameters,
                    operands,
                    binary.left,
                    binary.right,
                    &left,
                    &right,
                );
                closed_compare(&left, &right, binary.operator, format)
            }
            _ => None,
        },
        _ => match closed_scalar_value(program, parameters, operands, expression) {
            Some(ClosedScalar::Boolean(value)) => Some(value),
            _ => None,
        },
    }
}

/// Reduce one atom of a closed clause to a literal value. A contract formal's
/// name binds its operand expression and the recursion continues there; the
/// operand's own names are caller-local symbols, which never match a formal —
/// keeping substitution structurally finite and caller names uninterpreted.
fn closed_scalar_value(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<ClosedScalar> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_bignum().map(ClosedScalar::Integer),
        ExpressionNode::Float(value) => Some(ClosedScalar::Float(value.clone())),
        ExpressionNode::Boolean(value) => Some(ClosedScalar::Boolean(*value)),
        ExpressionNode::Borrow(borrow) => {
            closed_scalar_value(program, parameters, operands, borrow.target)
        }
        ExpressionNode::Name(path) => {
            let operand = operand_for_parameter(program, parameters, operands, path)?;
            closed_scalar_value(program, parameters, operands, operand)
        }
        _ => None,
    }
}

/// Bind a contract-side formal name to its operand expression. Self-alignment
/// mirrors the label instantiation: a captured receiver operand rides its
/// formal's ordinal; without one, operands count only the non-self formals.
fn operand_for_parameter(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    path: &typed_trees::expression::TableNamePath,
) -> Option<ExpressionHandle> {
    if !path.symbol.is_valid()
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let operands_include_self = operands.len() == parameters.len();
    let mut positional_operand_index = 0usize;
    for (parameter_index, parameter) in parameters.iter().enumerate() {
        let operand = if operands_include_self {
            operands.get(parameter_index)
        } else if parameter.is_self {
            None
        } else {
            let operand = operands.get(positional_operand_index);
            positional_operand_index += 1;
            operand
        };
        if path.symbol == parameter.symbol || path.head_symbol == parameter.symbol {
            return operand.copied();
        }
    }
    None
}

/// The float format a comparison reads at: a substituted formal's declared
/// type is authoritative (an unsuffixed literal reads at its operand type),
/// an explicitly suffixed literal decides otherwise, and f64 is the default
/// when no float context exists at all.
fn clause_comparison_format(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    left: ExpressionHandle,
    right: ExpressionHandle,
    left_scalar: &ClosedScalar,
    right_scalar: &ClosedScalar,
) -> FloatFormat {
    for side in [left, right] {
        let ExpressionNode::Name(path) = program.expression_table.expression(side) else {
            continue;
        };
        let Some(parameter) = parameters.iter().find(|parameter| {
            path.symbol == parameter.symbol || path.head_symbol == parameter.symbol
        }) else {
            continue;
        };
        if let Some(format) = program
            .type_reference_table
            .primitive_type(parameter.type_reference)
            .and_then(|primitive| match primitive {
                PrimitiveType::F32 => Some(FloatFormat::F32),
                PrimitiveType::F64 => Some(FloatFormat::F64),
                _ => None,
            })
        {
            return format;
        }
        // The operand's own literal landing is the next best witness of the
        // comparison's format (a `-8.75f32` operand stamps the clause f32).
        if let Some(operand) = operand_for_parameter(program, parameters, operands, path)
            && let ExpressionNode::Float(literal) = program.expression_table.expression(operand)
            && let Some(format) = literal.landing()
        {
            return format;
        }
    }
    [left_scalar, right_scalar]
        .iter()
        .find_map(|scalar| match scalar {
            ClosedScalar::Float(literal) => literal.landing(),
            _ => None,
        })
        .unwrap_or(FloatFormat::F64)
}

fn closed_compare(
    left: &ClosedScalar,
    right: &ClosedScalar,
    operator: BinaryOperator,
    format: FloatFormat,
) -> Option<bool> {
    match (left, right) {
        (ClosedScalar::Boolean(left), ClosedScalar::Boolean(right)) => match operator {
            BinaryOperator::Equal => Some(left == right),
            BinaryOperator::NotEqual => Some(left != right),
            _ => None,
        },
        (ClosedScalar::Integer(left), ClosedScalar::Integer(right)) => Some(match operator {
            BinaryOperator::Equal => left == right,
            BinaryOperator::NotEqual => left != right,
            BinaryOperator::Less => left < right,
            BinaryOperator::LessOrEqual => left <= right,
            BinaryOperator::Greater => left > right,
            BinaryOperator::GreaterOrEqual => left >= right,
            _ => return None,
        }),
        (ClosedScalar::Boolean(_), _) | (_, ClosedScalar::Boolean(_)) => None,
        _ => {
            let left = closed_exact_float(left, format)?;
            let right = closed_exact_float(right, format)?;
            Some(match operator {
                BinaryOperator::Equal => left.equal_value(&right),
                BinaryOperator::NotEqual => !left.equal_value(&right),
                BinaryOperator::Less => left.partial_cmp_value(&right) == Some(Ordering::Less),
                BinaryOperator::LessOrEqual => matches!(
                    left.partial_cmp_value(&right),
                    Some(Ordering::Less | Ordering::Equal)
                ),
                BinaryOperator::Greater => {
                    left.partial_cmp_value(&right) == Some(Ordering::Greater)
                }
                BinaryOperator::GreaterOrEqual => matches!(
                    left.partial_cmp_value(&right),
                    Some(Ordering::Greater | Ordering::Equal)
                ),
                _ => return None,
            })
        }
    }
}

/// Read one atom at the comparison's float format. An integer literal in a
/// float comparison rounds exactly to that format — the same landing a
/// written `0` takes against an f32 operand — and a float literal is decoded
/// from its exact IEEE meaning so NaN orderings stay unordered.
fn closed_exact_float(scalar: &ClosedScalar, format: FloatFormat) -> Option<ExactFloat> {
    match scalar {
        ClosedScalar::Float(literal) => Some(match format {
            FloatFormat::F32 => ExactFloat::from_f32(literal.value_f32()),
            FloatFormat::F64 => ExactFloat::from_f64(literal.value_f64()),
        }),
        ClosedScalar::Integer(value) => {
            let rational = BigRational::from_integer(value.clone());
            Some(match format {
                FloatFormat::F32 => ExactFloat::from_f32(
                    rational.to_f32_with_rounding(IeeeRounding::NearestTiesToEven),
                ),
                FloatFormat::F64 => ExactFloat::from_f64(
                    rational.to_f64_with_rounding(IeeeRounding::NearestTiesToEven),
                ),
            })
        }
        ClosedScalar::Boolean(_) => None,
    }
}

/// Every operator occurrence inside a closed leaf must resolve to a builtin
/// meaning — builtin fallback, or a body-less `boundary machine` realized by
/// the target (such as `Float::equal`) — before its literal atoms may be
/// evaluated. A checked or boundary-operator selection keeps the label/fact
/// path: its meaning is not inferred from the spelling here.
fn closed_operators_are_builtin(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> bool {
    if operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && !operator_use_is_builtin_meaning(program, operators, operator_use)
    }) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            closed_operators_are_builtin(program, operators, binary.left)
                && closed_operators_are_builtin(program, operators, binary.right)
        }
        ExpressionNode::Unary(unary) => {
            closed_operators_are_builtin(program, operators, unary.operand)
        }
        ExpressionNode::Borrow(borrow) => {
            closed_operators_are_builtin(program, operators, borrow.target)
        }
        _ => true,
    }
}

fn operator_use_is_builtin_meaning(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    operator_use: &checked_trees::CheckedOperatorUseFact,
) -> bool {
    match operator_use.status {
        CheckedOperatorResolutionStatus::BuiltinFallback => true,
        CheckedOperatorResolutionStatus::Resolved => operators
            .selected_candidate(operator_use)
            .is_some_and(|selected| {
                program.machine_token_bindings().iter().any(|binding| {
                    binding.symbol == selected.operator_symbol && binding.is_boundary
                })
            }),
        _ => false,
    }
}

/// A `subject in T::C` leaf -- lowered to `subject == T::C` -- is proven by
/// the installed case at the operand place the subject names. The assigned
/// tag, not a value comparison, is the evidence.
fn contexts_prove_case_membership(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let Some((subject, case)) = crate::proof::exact_outcome_case_test(program, expression) else {
        return false;
    };
    let Some(formal) = crate::flow::canonical_place_from_expression(program, subject) else {
        return false;
    };
    let facts::PlaceRoot::Symbol(symbol) = formal.root else {
        return false;
    };
    let Some(position) = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.symbol == symbol)
    else {
        return false;
    };
    let Some(mut place) = operands
        .get(position)
        .and_then(|operand| crate::flow::canonical_place_from_expression(program, *operand))
    else {
        return false;
    };
    place.segments.extend(formal.segments);
    let contexts = contexts.for_expressions(program, parameters, [expression]);
    contexts.iter().any(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
            .any(|fact| {
                matches!(fact.payload, FactPayload::AssignedCase { variant } if variant == case)
                    && matches!(fact.place, FactPlace::Place(candidate) if
                    crate::flow::canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(candidate),
                    )
                    .is_some_and(|candidate| {
                        crate::flow::normalized_event_place_root(program, candidate.root)
                            == crate::flow::normalized_event_place_root(program, place.root)
                            && candidate.segments == place.segments
                    }))
            })
    })
}
