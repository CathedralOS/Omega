use omega_checked_trees::{
    CheckedOperatorCandidateFact, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedOperatorUseFact, CheckedValueFacts, CheckedValueOrigin,
};
use omega_core::arena::Arena;
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::operator::{SpelledOperator, resolve_spelling};
use omega_typed_trees::types::TypeReferenceHandle;

mod receiver;

use receiver::expression_type_reference_for_origin;

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
