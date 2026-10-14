//! Exact direct-reference results share the frozen-prefix result-leaf query.
//! The result has one reference leaf, not a separate body-analysis rule.

use super::super::{TableCallExpression, machine_state_by_symbol, result_origins};
use super::{
    ExpressionHandle, FrameInference, FramePathPrecision, FramePlaceOrigin, Machine,
    StoredLocalOrigins, TopLevelSymbols, caller_aliases, stored_origins,
    type_reference_is_reference, value_origins,
};
pub(super) fn call_expression(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> Option<&TableCallExpression> {
    expression = program
        .expression_table
        .peeled_expression_handle(expression);
    let call = program.expression_table.call_node(expression)?;
    Some(call)
}

pub(super) fn call_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    let call = call_expression(program, expression)?;
    let (_, callee_state) = machine_state_by_symbol(program, call.target_symbol)?;
    if !type_reference_is_reference(program, callee_state.return_type) {
        return None;
    }
    let (state, before, _) = caller_aliases::caller_statement_at_site(
        program,
        machine,
        symbols,
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
            value_origins(
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
    if !returned.cases.is_empty() || !returned.moves.is_empty() || returned.references.is_empty() {
        return None;
    }
    returned
        .references
        .into_iter()
        .map(|leaf| {
            let origin = leaf.origin?;
            (leaf.kind == stored_origins::StoredOriginKind::Reference
                && leaf.local_segments.is_empty()
                && leaf.local_suffix.is_empty()
                && origin.precision == FramePathPrecision::Exact
                && origin.source.root.is_valid())
            .then_some(origin)
        })
        .collect()
}
