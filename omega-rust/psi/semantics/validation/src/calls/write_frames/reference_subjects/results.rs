//! Exact direct-reference results share the frozen-prefix result-leaf query.
//! The result has one reference leaf, not a separate body-analysis rule.

use super::*;

pub(super) fn call_expression(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> Option<&TableCallExpression> {
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        expression = borrow.target;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    Some(call)
}

pub(super) fn call_origin(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    let call = call_expression(program, expression)?;
    let (_, callee_state) = machine_state_by_symbol(program, call.target_symbol)?;
    if !type_reference_is_reference(program, callee_state.return_type) {
        return None;
    }
    let (state, before, _) = caller_aliases::caller_statement_at_site(
        program,
        machine,
        caller_aliases::CallerWriteSite::Expression(expression),
    )?;
    let returned = result_origins::call_result_origins(
        program,
        machine,
        call,
        callee_state.return_type,
        symbols,
        inference,
        true,
        &|actual, _, implicit_borrow, inference| {
            value_origin(
                program,
                machine,
                actual,
                symbols,
                inference,
                aliases,
                stored,
                implicit_borrow,
            )
        },
        &|actual, reference, _| {
            stored_origins::reference_leaves_before_statement_for_query(
                program,
                state,
                before,
                actual,
                reference,
                Some(stored),
                None,
                true,
            )
        },
    )?;
    let [leaf] = returned.references.as_slice() else {
        return None;
    };
    (returned.cases.is_empty()
        && returned.moves.is_empty()
        && leaf.local_segments.is_empty()
        && leaf.local_suffix.is_empty()
        && leaf.origin.precision == FramePathPrecision::Exact
        && leaf.origin.source.root.is_valid())
    .then(|| leaf.origin.clone())
}
