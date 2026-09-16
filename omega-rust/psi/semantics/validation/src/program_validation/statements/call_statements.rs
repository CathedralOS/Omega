//! A call statement: the callee's operands, custody transfer and any
//! boundary operator application it validates.

use super::{StatementOutputs, StatementScope};
use crate::machine_calls::calls::validate_call_node;
use crate::proof_contracts::arithmetic_domains;
use typed_trees::statement::StatementNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
    statement_handle: typed_trees::statement::StatementHandle,
    direct_written: Option<Vec<String>>,
) {
    let StatementNode::Call(call) = statement else {
        unreachable!("dispatched call_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        ..
    } = *scope;
    let value_env = &mut *outputs.value_env;
    let boundary_operator_applications = &mut *outputs.boundary_operator_applications;
    let diagnostics = &mut *outputs.diagnostics;
    if let Some(state) = current_state {
        crate::declarations::operators::validate_named_statement_operator_application(
            program,
            symbols,
            machine,
            state,
            statement_handle,
            call,
            boundary_operator_applications,
            diagnostics,
        );
    }
    validate_call_node(
        program,
        call,
        machine,
        state_name,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        value_env,
        diagnostics,
    );
    // R5 frame seed: a resolved acyclic INTERNAL call preserves facts
    // outside its conservatively instantiated may-write set. Unknown,
    // unsummarized, and overlapping implementations remain
    // conservative. Authored `stores` clauses are retired; exactness
    // grows through inferred implementation summaries.
    if let Some(written) = direct_written {
        value_env.invalidate_written_paths(&written);
    } else {
        value_env.clear();
    }
    // R4 witness mint: a BOUNDARY callee's `ensures` re-seeds the
    // `&mut` out-arguments' places (the boundary model's citable
    // fact) -- `fw.get_size(&mut self.n)` with `ensures size <= 8`
    // leaves `self.n` in [type_low, 8].
    if let Some(signature) = crate::machine_calls::calls::boundary_trait_signature(
        program,
        machine,
        machine_symbols,
        symbols,
        call,
    ) {
        arithmetic_domains::seed_out_param_ensures(
            program,
            machine,
            current_state,
            call,
            signature,
            value_env,
        );
    }
}
