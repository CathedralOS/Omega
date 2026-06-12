//! Use-site discharge for `requires` contracts carried by the SELECTED meaning
//! of a spelled BINARY operator use.
//!
//! Model: the bounds-from-`requires` seam for `[]`/`[..]`
//! (checks/ranges/indexes/validation.rs) sources the access obligation from
//! the spelled operator's `requires` contract and discharges it with the
//! bounds prover. Spelled binary uses have no special-purpose prover, so the
//! obligation is INSTANTIATED over the actual operands (parameter -> operand,
//! the call-`requires` instantiation precedent in
//! checks/contracts/labels/calls.rs) and proven against the semantic contexts
//! entering the use's statement — the same invalidation-adjusted contexts the
//! selection pass reads, so caller `requires`, call `ensures`, and
//! interleaved-mutation invalidation all participate.
//!
//! An unproven obligation reports the operator-contract attribution shape the
//! indexed seam established: name the instantiated clause, the operator that
//! declares it, and the spelling that resolved to it, so the user can browse
//! to the operator declaration and read the governing contract.

use omega_checked_trees::{CheckFacts, CheckedValueOrigin, FlowStateFact};
use omega_core::diagnostics::Diagnostic;
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactContextHandle, FactPayload, FactPlace, FactPlan};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::operator::OperatorDefinition;
use omega_typed_trees::signature::{SignatureContractKind, StateParameter};

use super::super::contracts::labels::domain_proves_expression_label;
use crate::labels::{canonical_place_label, symbol_name};

/// Checks every `requires` contract of every selected spelled binary meaning
/// against the facts entering the use's statement, reporting each unproven
/// clause. Slice `[]`/`[..]` uses discharge through the ranges seam and are
/// deliberately excluded.
pub(super) fn selected_binary_requires_diagnostics(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for operator_use in facts.operators.resolved_uses() {
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
        let operands = binary_operands(program, operator_use.expression);
        // Non-statement origins (contract expressions, decreases, data
        // initializers) carry no flow context, so nothing can prove the
        // contract there: every clause reports unproven.
        let entry_contexts = statement_entry_contexts(facts, operator_use.origin);

        for fact in requires_facts {
            let proven = entry_contexts.as_deref().is_some_and(|contexts| {
                requires_fact_proven(
                    program,
                    &facts.semantic,
                    contexts,
                    parameters,
                    &operands,
                    fact,
                )
            });
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

/// The use's binary operands in parameter order: a spelled binary operator's
/// first parameter binds the left operand and the second the right.
fn binary_operands(program: &TypedTrees, expression: ExpressionHandle) -> Vec<ExpressionHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
        _ => Vec::new(),
    }
}

/// The semantic contexts entering the use's statement — the same
/// invalidation-adjusted contexts the selection pass and the call-`requires`
/// discharge read. `None` when the use has no statement flow context.
fn statement_entry_contexts(
    facts: &CheckFacts,
    origin: CheckedValueOrigin,
) -> Option<Vec<FactContextHandle>> {
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        ..
    } = origin
    else {
        return None;
    };
    let state_flow: &FlowStateFact = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| {
            state.machine_symbol == machine_symbol && state.state_symbol == state_symbol
        })?;
    let entry_constraints = facts
        .flow
        .state_statement(state_flow, statement_index)
        .map(|statement| statement.entry_constraints)
        .unwrap_or(state_flow.entry_constraints);
    Some(
        facts
            .flow
            .semantic_constraint_contexts(entry_constraints)
            .collect(),
    )
}

/// Whether one instantiated `requires` fact is proven by any context entering
/// the statement.
fn requires_fact_proven(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    fact: &ProofFact,
) -> bool {
    match fact {
        ProofFact::Membership(membership) => {
            let value_label = instantiate_operator_expression_label(
                program,
                parameters,
                operands,
                membership.value,
            );
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
    }
}

/// Boolean `requires` clauses decompose like the call-`requires` prover:
/// conjunctions need both sides, disjunctions either side, and a leaf is
/// proven by a matching instantiated fact (directly, or derived from a domain
/// membership whose body states the clause).
fn contexts_prove_boolean_expression(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => contexts_prove_boolean_expression(
            program, semantic, contexts, parameters, operands, *inner,
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
    contexts: &[FactContextHandle],
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let required_label =
        instantiate_operator_expression_label(program, parameters, operands, expression);
    contexts
        .iter()
        .any(|context| context_proves_boolean_label(program, semantic, *context, &required_label))
}

/// A leaf clause is proven by a boolean fact whose rendered form matches the
/// instantiated clause, or by a domain membership whose domain body states it
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
            FactPayload::BooleanExpression(candidate)
            | FactPayload::ContractBooleanExpression {
                expression: candidate,
                ..
            } => program.expression_table.display_name(candidate) == required_label,
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
        if !semantic.domain_implies(fact_domain, required_domain) {
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
        ProofFact::Expression(expression) => {
            instantiate_operator_expression_label(program, parameters, operands, *expression)
        }
        ProofFact::Membership(membership) => format!(
            "{} in {}",
            instantiate_operator_expression_label(program, parameters, operands, membership.value),
            symbol_name(program, membership.domain_symbol)
        ),
    }
}

/// Renders an operator contract expression with each parameter reference
/// substituted by its operand's label — the operator-use analogue of
/// `instantiate_call_contract_expression_label` (parameter -> argument) in
/// checks/contracts/labels/calls.rs. Operators have no `self` and no `result`
/// binder; parameters map positionally onto the binary operands.
fn instantiate_operator_expression_label(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> String {
    let instantiate = |expression: ExpressionHandle| {
        instantiate_operator_expression_label(program, parameters, operands, expression)
    };

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            let values = program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| instantiate(*value))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            instantiate(binary.left),
            binary.operator.display_name(),
            instantiate(binary.right)
        ),
        ExpressionNode::Boolean(value) => value.to_string(),
        ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate(cast.value),
            omega_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(cast.target_type),
                "::",
            )
        ),
        ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| instantiate(*argument))
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    instantiate(call.receiver),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        ExpressionNode::Float(value) => value.to_string(),
        ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            instantiate(indexed.collection),
            instantiate(indexed.index)
        ),
        ExpressionNode::Range(range) => match (range.start.is_valid(), range.end.is_valid()) {
            (true, true) => format!("{}..{}", instantiate(range.start), instantiate(range.end)),
            (true, false) => format!("{}..", instantiate(range.start)),
            (false, true) => format!("..{}", instantiate(range.end)),
            (false, false) => "..".to_owned(),
        },
        ExpressionNode::Integer(value) => value.to_string(),
        ExpressionNode::Member(member) => {
            format!("{}.{}", instantiate(member.receiver), member.member)
        }
        ExpressionNode::Mutable(inner) => format!("mut {}", instantiate(*inner)),
        ExpressionNode::Unary(unary) => format!(
            "{}{}",
            unary.operator.display_name(),
            instantiate(unary.operand)
        ),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let first_member = members.first().map(|member| member.as_str());
            let mut operand_index = 0usize;

            for parameter in parameters {
                if parameter.is_self {
                    continue;
                }
                let operand = operands.get(operand_index).copied();
                operand_index = operand_index.saturating_add(1);

                let parameter_matches = first_member == Some(parameter.name.as_str())
                    || path.head_symbol == parameter.symbol
                    || path.symbol == parameter.symbol;
                if parameter_matches {
                    return operand
                        .map(|operand| program.expression_table.display_name(operand))
                        .unwrap_or_else(|| parameter.name.to_string());
                }
            }

            omega_typed_trees::expression::display_name_path(members, "::")
        }
        ExpressionNode::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
        ExpressionNode::String(value) => format!("{value:?}"),
    }
}
