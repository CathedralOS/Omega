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
    let value_environment = &mut *outputs.value_environment;
    let exact_integer_casts = &mut *outputs.exact_integer_casts;
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
        value_environment,
        diagnostics,
    );
    // The operands were validated under this statement's flow environment;
    // retain every accepted exact fixed-integer cast among them before the
    // call's writes disturb that environment, exactly as an expression-
    // position call does. Checked lowering replays a call argument such as
    // `value as u8` from this evidence rather than from ambient trust.
    for argument in program.statement_table.expression_handles(call.arguments) {
        arithmetic_domains::collect_exact_integer_cast_facts(
            program,
            machine,
            current_state,
            *argument,
            value_environment,
            exact_integer_casts,
        );
    }
    // R5 frame seed: a resolved acyclic INTERNAL call preserves facts
    // outside its conservatively instantiated may-write set. Unknown,
    // unsummarized, and overlapping implementations remain
    // conservative. Authored `stores` clauses are retired; exactness
    // grows through inferred implementation summaries.
    if let Some(written) = direct_written {
        value_environment.invalidate_written_paths(&written);
    } else {
        value_environment.clear();
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
            value_environment,
        );
    }
}
