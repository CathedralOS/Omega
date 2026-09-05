//! Capture a selected initializer or assignment while its operand facts are live.

use super::*;
use psi_checked_trees::CheckedScalarExpressionRole;
use psi_facts::ScalarValue;

pub(super) fn capture_statement(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    context: &FlowBuildContext,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<ScalarValue> {
    let source = match statement {
        StatementNode::LocalData(local) => local.initial_value,
        StatementNode::Assignment(assignment) => assignment.value,
        _ => return None,
    };
    if !program.expression_table.expression_is_valid(source) {
        return None;
    }
    let statement_ordinal = u32::try_from(statement_index).ok()?;
    let plans = context.scalar_expressions;
    let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
        binding.state == state
            && binding.statement_ordinal == statement_ordinal
            && binding.expression == source
            && match statement {
                StatementNode::LocalData(local) if local.is_mutable => {
                    binding.role == CheckedScalarExpressionRole::StorageInitializer
                }
                StatementNode::LocalData(_) => matches!(
                    binding.role,
                    CheckedScalarExpressionRole::LocalInitializer { .. }
                ),
                StatementNode::Assignment(_) => {
                    binding.role == CheckedScalarExpressionRole::AssignmentValue
                }
                _ => false,
            }
    });
    let (_, binding) = bindings.next()?;
    if bindings.next().is_some() {
        return None;
    }
    let mut expressions = plans.expressions.iter().filter(|expression| {
        expression.state == state
            && expression.statement_ordinal == statement_ordinal
            && expression.role == binding.role
    });
    let expression = &expressions.next()?.expression;
    if expressions.next().is_some() {
        return None;
    }
    let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
    crate::values::evaluate_checked_scalar(expression, &mut |position| {
        let place = canonical_place_from_symbol(*symbols.get(position)?)?;
        crate::values::scalar_value_at_place(
            program,
            semantic,
            context
                .contexts
                .semantic_context_refs
                .span_or_empty(active)
                .iter()
                .map(|reference| semantic.contexts.get(reference.context)),
            &place,
        )
    })
}
