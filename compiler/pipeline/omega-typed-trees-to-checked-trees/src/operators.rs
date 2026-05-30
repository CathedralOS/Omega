use omega_checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    CheckedValueFacts,
};
use omega_core::arena::Arena;
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::operator::OperatorDefinition;

pub(crate) fn build_operator_facts(
    program: &TypedTrees,
    values: &CheckedValueFacts,
) -> CheckedOperatorFacts {
    let mut uses = Arena::default();
    let mut candidate_symbols = Arena::default();
    let mut seen = Vec::new();

    for (_, value) in values.values.iter() {
        collect_expression_operator_use(
            program,
            value.expression,
            &mut seen,
            &mut uses,
            &mut candidate_symbols,
        );
    }

    CheckedOperatorFacts::with_roots(uses, candidate_symbols)
}

fn collect_expression_operator_use(
    program: &TypedTrees,
    expression: ExpressionHandle,
    seen: &mut Vec<ExpressionHandle>,
    uses: &mut Arena<CheckedOperatorUseFact>,
    candidate_symbols: &mut Arena<SymbolHandle>,
) {
    if !expression.is_valid() || seen.iter().any(|seen| *seen == expression) {
        return;
    }
    seen.push(expression);

    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            let spelling = indexed_operator_spelling(program, indexed.index);
            uses.append(operator_use_fact(
                program,
                expression,
                spelling,
                candidate_symbols,
            ));
            collect_expression_operator_use(
                program,
                indexed.collection,
                seen,
                uses,
                candidate_symbols,
            );
            collect_expression_operator_use(program, indexed.index, seen, uses, candidate_symbols);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_operator_use(program, *value, seen, uses, candidate_symbols);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_operator_use(program, binary.left, seen, uses, candidate_symbols);
            collect_expression_operator_use(program, binary.right, seen, uses, candidate_symbols);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_operator_use(program, cast.value, seen, uses, candidate_symbols);
        }
        ExpressionNode::Call(call) => {
            collect_expression_operator_use(program, call.receiver, seen, uses, candidate_symbols);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_operator_use(program, *argument, seen, uses, candidate_symbols);
            }
        }
        ExpressionNode::Member(member) => {
            collect_expression_operator_use(
                program,
                member.receiver,
                seen,
                uses,
                candidate_symbols,
            );
        }
        ExpressionNode::Mutable(inner) => {
            collect_expression_operator_use(program, *inner, seen, uses, candidate_symbols);
        }
        ExpressionNode::Range(range) => {
            collect_expression_operator_use(program, range.start, seen, uses, candidate_symbols);
            collect_expression_operator_use(program, range.end, seen, uses, candidate_symbols);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_operator_use(
                    program,
                    field.value,
                    seen,
                    uses,
                    candidate_symbols,
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

fn operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    spelling: OperatorSpelling,
    candidate_symbols: &mut Arena<SymbolHandle>,
) -> CheckedOperatorUseFact {
    let candidates = operator_candidates_by_spelling(program, spelling);
    let candidate_count = candidates.len();
    let candidate_symbols_span =
        candidate_symbols.insert_many(candidates.iter().map(|candidate| candidate.symbol));
    let selected_operator_symbol = match candidates.as_slice() {
        [candidate] => candidate.symbol,
        _ => SymbolHandle::invalid(),
    };
    let status = match candidate_count {
        0 => CheckedOperatorResolutionStatus::Missing,
        1 => CheckedOperatorResolutionStatus::Resolved,
        _ => CheckedOperatorResolutionStatus::Ambiguous,
    };

    CheckedOperatorUseFact {
        expression,
        spelling,
        selected_operator_symbol,
        candidate_symbols: candidate_symbols_span,
        candidate_count,
        status,
    }
}

fn operator_candidates_by_spelling(
    program: &TypedTrees,
    spelling: OperatorSpelling,
) -> Vec<&OperatorDefinition> {
    program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain).iter()),
        )
        .filter(|operator| operator.spelling == Some(spelling))
        .collect()
}
