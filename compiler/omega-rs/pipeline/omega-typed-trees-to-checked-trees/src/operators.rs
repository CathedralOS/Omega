use omega_checked_trees::{
    CheckedOperatorCandidateFact, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedOperatorUseFact, CheckedValueFacts, CheckedValueOrigin,
};
use omega_core::arena::Arena;
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::operator::{SpelledOperator, resolve_spelling};
use omega_typed_trees::types::TypeReferenceHandle;

mod receiver;
mod selection;

use receiver::expression_type_reference_for_origin;
pub(crate) use selection::select_pending_domain_operator_meanings;

pub(crate) fn build_operator_facts(
    program: &TypedTrees,
    values: &CheckedValueFacts,
) -> CheckedOperatorFacts {
    let mut uses = Arena::default();
    let mut candidates = Arena::default();
    let mut seen = Vec::new();

    for (_, value) in values.values.iter() {
        collect_expression_operator_use(
            program,
            value.expression,
            value.origin,
            &mut seen,
            &mut uses,
            &mut candidates,
        );
    }

    CheckedOperatorFacts::with_roots(uses, candidates)
}

fn collect_expression_operator_use(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    seen: &mut Vec<(ExpressionHandle, CheckedValueOrigin)>,
    uses: &mut Arena<CheckedOperatorUseFact>,
    candidates: &mut Arena<CheckedOperatorCandidateFact>,
) {
    if !expression.is_valid() || seen.iter().any(|seen| *seen == (expression, origin)) {
        return;
    }
    seen.push((expression, origin));

    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            let spelling = indexed_operator_spelling(program, indexed.index);
            let receiver_type =
                expression_type_reference_for_origin(program, indexed.collection, origin);
            uses.append(operator_use_fact(
                program,
                expression,
                origin,
                spelling,
                receiver_type,
                candidates,
            ));
            collect_expression_operator_use(
                program,
                indexed.collection,
                origin,
                seen,
                uses,
                candidates,
            );
            collect_expression_operator_use(program, indexed.index, origin, seen, uses, candidates);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_operator_use(program, *value, origin, seen, uses, candidates);
            }
        }
        ExpressionNode::Binary(binary) => {
            // A spelled binary use participates in operator resolution only
            // when the left operand type is known and at least one spelled
            // candidate matches it: builtin-only arithmetic stays unrecorded
            // (and untouched), exactly as before spelled binary dispatch.
            if let Some(spelling) = binary_operator_spelling(binary.operator)
                && let Some(receiver_type) =
                    expression_type_reference_for_origin(program, binary.left, origin)
                && let Some(fact) = binary_operator_use_fact(
                    program,
                    expression,
                    origin,
                    spelling,
                    receiver_type,
                    candidates,
                )
            {
                uses.append(fact);
            }
            collect_expression_operator_use(program, binary.left, origin, seen, uses, candidates);
            collect_expression_operator_use(program, binary.right, origin, seen, uses, candidates);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_operator_use(program, cast.value, origin, seen, uses, candidates);
        }
        ExpressionNode::Call(call) => {
            collect_expression_operator_use(program, call.receiver, origin, seen, uses, candidates);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_operator_use(program, *argument, origin, seen, uses, candidates);
            }
        }
        ExpressionNode::Member(member) => {
            collect_expression_operator_use(
                program,
                member.receiver,
                origin,
                seen,
                uses,
                candidates,
            );
        }
        ExpressionNode::Mutable(inner) => {
            collect_expression_operator_use(program, *inner, origin, seen, uses, candidates);
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_operator_use(program, unary.operand, origin, seen, uses, candidates);
        }
        ExpressionNode::Range(range) => {
            collect_expression_operator_use(program, range.start, origin, seen, uses, candidates);
            collect_expression_operator_use(program, range.end, origin, seen, uses, candidates);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_operator_use(
                    program,
                    field.value,
                    origin,
                    seen,
                    uses,
                    candidates,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// The fixed operator spelling for a binary operator, when one exists.
/// Logical/shift operators have no spelling surface (frozen Wave 0 decision
/// #3) and never participate in spelled dispatch.
fn binary_operator_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
    Some(match operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return None,
    })
}

/// Records a spelled binary use when spelled candidates match the left
/// operand type. Root-only candidate sets resolve immediately (root spelled
/// operators are the declared surface of the builtin operation). Any
/// domain-owned candidate defers to the post-flow selection pass
/// (`select_pending_domain_operator_meanings`): chapter 8 admits a domain
/// meaning only when the operand's domain membership is PROVEN in the current
/// proof context, and proof contexts do not exist yet at this stage.
fn binary_operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    receiver_type: TypeReferenceHandle,
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> Option<CheckedOperatorUseFact> {
    let candidates = resolve_spelling(program, spelling, Some(receiver_type));
    if candidates.is_empty() {
        return None;
    }

    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(
        candidates
            .iter()
            .map(|candidate| checked_candidate(program, candidate)),
    );
    let (status, selected_operator_symbol) = if candidates
        .iter()
        .any(|candidate| candidate.domain.is_some())
    {
        (
            CheckedOperatorResolutionStatus::DomainPending,
            SymbolHandle::invalid(),
        )
    } else if let [candidate] = candidates.as_slice() {
        (
            CheckedOperatorResolutionStatus::Resolved,
            candidate.operator.symbol,
        )
    } else {
        (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        )
    };

    Some(CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    })
}

fn indexed_operator_spelling(program: &TypedTrees, index: ExpressionHandle) -> OperatorSpelling {
    if index.is_valid()
        && matches!(
            program.expression_table.expression(index),
            ExpressionNode::Range(_)
        )
    {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    }
}

/// Records the typed-trees resolution outcome for one use site as checked
/// evidence: the resolution itself is `omega_typed_trees::operator::resolve_spelling`.
fn operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    receiver_type: Option<TypeReferenceHandle>,
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> CheckedOperatorUseFact {
    let candidates = resolve_spelling(program, spelling, receiver_type);
    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(
        candidates
            .iter()
            .map(|candidate| checked_candidate(program, candidate)),
    );
    let selected_operator_symbol = match candidates.as_slice() {
        [candidate] => candidate.operator.symbol,
        _ => SymbolHandle::invalid(),
    };
    let status = match candidate_count {
        0 => CheckedOperatorResolutionStatus::Missing,
        1 => CheckedOperatorResolutionStatus::Resolved,
        _ => CheckedOperatorResolutionStatus::Ambiguous,
    };

    CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    }
}

fn checked_candidate(
    program: &TypedTrees,
    candidate: &SpelledOperator<'_>,
) -> CheckedOperatorCandidateFact {
    let fact = if let Some(domain) = candidate.domain {
        CheckedOperatorCandidateFact::domain(candidate.operator.symbol, domain.symbol)
    } else {
        CheckedOperatorCandidateFact::root(candidate.operator.symbol)
    };
    fact.with_signature(
        program
            .operator_parameters(candidate.operator)
            .first()
            .map(|parameter| parameter.type_reference)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        candidate.operator.return_type,
        candidate.operator.contracts,
        program.operator_type_parameters(candidate.operator).len(),
        program.operator_parameters(candidate.operator).len(),
        candidate.operator.is_boundary,
    )
}
