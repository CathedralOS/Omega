//! Per-context Boolean fact queries shared by selected operator requires
//! and named crash-route discharge. Callers select contexts, polarity and
//! incoming rows; this module only matches a retained true fact's label.
//! Payload-domain expansion additionally requires selected cases in that exact
//! incoming row and consumer scope; conditional validity is not a true clause.

use facts::{FactContextHandle, FactPayload, FactPlan};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::semantic_boolean_fact_label;
use crate::checks::contracts::labels::domain_proves_expression_label;

/// A retained true clause or domain requirement proves an exact rendered leaf.
pub(crate) fn context_proves_boolean_label(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    context: FactContextHandle,
    contexts: &[FactContextHandle],
    point: facts::ProgramPoint,
    required_label: &str,
) -> bool {
    let availability = crate::facts::boolean_predicates::CasePremiseAvailability::new(
        program, semantic, contexts, point,
    );
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
                let Some(place) = fact.place.as_place() else {
                    return false;
                };
                let Some(subject) = crate::flow::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                ) else {
                    return false;
                };
                if !availability.allows(&subject) {
                    return false;
                }
                let canonical_base = semantic.place_label(program, place);
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
    program: &dyn ::validation::CheckingInput,
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
