//! Closed extent expressions rejoin the existing length-job continuation.
//! Private entries own no execution grant. Their exact original roots, lexical
//! selections, integer carriers and consuming exposure survive ordinary typing,
//! dependency settlement, checking, admission and selected-provider evaluation.

use crate::machine_execution::admission::require_closed_expression_custody;
use diagnostics::Diagnostic;
use source::SourceSpan;
use symbols::SymbolHandle;
use syntax_trees::{
    SyntaxTrees,
    expression::ExpressionHandle,
    identifier::Identifier,
    types::{ConstArgumentOrigin, TypeReferenceHandle},
};
use syntax_trees_to_symbol_resolved_trees::pre_resolution::retain_deferred_array_extent;
use typed_trees::TypedTrees;
use validation::expression_result_type_reference;

type Application = (
    TypeReferenceHandle,
    ExpressionHandle,
    TypeReferenceHandle,
    bool,
);
type LexicalSelection = (
    TypeReferenceHandle,
    Vec<ConstArgumentOrigin>,
    Vec<(SourceSpan, SourceSpan)>,
);

pub(super) fn retain(
    syntax: &mut SyntaxTrees,
    execution: &TypedTrees,
    applications: &[Application],
    owners: &[SymbolHandle],
    extents: &[TypeReferenceHandle],
    lexical_selections: &[LexicalSelection],
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<bool, Vec<Diagnostic>> {
    let mut retained = false;
    for ((argument, expression, _, public), owner) in applications.iter().zip(owners) {
        if !extents.contains(argument) {
            continue;
        }
        let reference = syntax.expressions.source_span(*expression);
        let failure = |reason: String| {
            vec![
                Diagnostic::error(format!("array extent entry retention failed: {reason}"))
                    .with_source_span(reference),
            ]
        };
        let machine = execution
            .machine(*owner)
            .ok_or_else(|| failure("extent probe lost its exact declaration".into()))?;
        let [state] = execution.machine_states(machine) else {
            return Err(failure("extent probe lost its state".into()));
        };
        let [typed_trees::statement::StatementNode::Transition(transition)] =
            execution.statement_table.statements(state.statement_nodes)
        else {
            return Err(failure("extent probe lost its return".into()));
        };
        let typed_trees::statement::TransitionTargetNode::Value(root) = execution
            .statement_table
            .transition_target(transition.target)
        else {
            return Err(failure("extent probe return is not a value".into()));
        };
        // Original public exposure and lexical selections are checked
        // before retention, independently of private entry visibility.
        let (origins, _, calls) =
            super::super::const_generic_expressions::pending_leaf_call_custody(
                execution,
                machine,
                state,
                &[*root],
                *public,
                syntax,
            )
            .map_err(&failure)?;
        let (_, expected_origins, expected_calls) = lexical_selections
            .iter()
            .find(|(original, ..)| original == argument)
            .ok_or_else(|| failure("extent job lost lexical custody".into()))?;
        if origins.len() != expected_origins.len()
            || origins
                .iter()
                .any(|origin| !expected_origins.contains(origin))
            || calls.len() != expected_calls.len()
            || calls.iter().any(|call| !expected_calls.contains(call))
        {
            return Err(failure(
                "extent entry changed original declaration selections".into(),
            ));
        }
        require_closed_expression_custody(execution, *root, authority).map_err(&failure)?;
        let (natural, policy) = expression_result_type_reference(execution, machine, state, *root)
            .and_then(|result| {
                super::super::const_generic_expressions::extent_integer_destination(
                    execution, result,
                )
            })
            .ok_or_else(|| {
                failure(
                    "array extent entry has no supported integer result carrier and policy".into(),
                )
            })?;
        let base = syntax
            .type_references
            .insert_named(Identifier::new(natural.name(), reference));
        let destination = if policy == numerics::arithmetic::ArithmeticDomain::Exact {
            base
        } else {
            let constraint = syntax.type_references.append_constraint(
                syntax_trees::types::TypeConstraintNode::ArithmeticDomain(policy),
            );
            syntax
                .type_references
                .insert_constrained(base, arena::HandleSpan::from_parts(constraint, 1))
        };
        let entry = super::super::const_generic_expressions::append_extent_probe(
            syntax,
            *argument,
            *expression,
            destination,
            *public,
        );
        retain_deferred_array_extent(syntax, *argument, entry);
        retained = true;
    }
    Ok(retained)
}
