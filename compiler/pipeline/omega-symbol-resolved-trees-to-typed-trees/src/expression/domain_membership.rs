use super::lower_expression_handle_from_table_with_self_substitution;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(super) fn lower_domain_membership_expression(
    program: &resolved::SymbolResolvedTrees,
    target: &mut typed::expression::ExpressionTable,
    value: typed::expression::ExpressionHandle,
    domain_symbol: omega_core::symbols::SymbolHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let Some(domain_definition) = program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return Err(Diagnostic::error(format!(
            "cannot lower executable membership for unknown domain symbol {}",
            domain_symbol.arena_index()
        )));
    };

    let source = &program.tables.bodies.expressions;
    let mut lowered_facts = Vec::new();
    for fact in program.proof_facts(domain_definition.facts) {
        let lowered = match fact {
            resolved::domain::ProofFact::Expression(expression) => {
                lower_expression_handle_from_table_with_self_substitution(
                    Some(program),
                    source,
                    target,
                    *expression,
                    Some(value),
                )?
            }
            resolved::domain::ProofFact::Membership(membership) => {
                let nested_value = lower_expression_handle_from_table_with_self_substitution(
                    Some(program),
                    source,
                    target,
                    membership.value,
                    Some(value),
                )?;
                lower_domain_membership_expression(
                    program,
                    target,
                    nested_value,
                    membership.domain_symbol,
                )?
            }
        };
        lowered_facts.push(lowered);
    }

    let mut lowered_facts = lowered_facts.into_iter();
    let Some(mut combined) = lowered_facts.next() else {
        return Ok(target.insert(typed::expression::ExpressionNode::Boolean(true)));
    };

    for fact in lowered_facts {
        combined = target.insert(typed::expression::ExpressionNode::Binary(
            typed::expression::TableBinaryExpression {
                left: combined,
                operator: typed::expression::BinaryOperator::And,
                right: fact,
            },
        ));
    }

    Ok(combined)
}
