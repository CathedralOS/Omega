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
    let Some(call) = statement.as_call() else {
        unreachable!("dispatched call_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        symbols,
        writable_roots,
        ..
    } = *scope;
    let value_environment = &mut *outputs.value_environment;
    let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
    let boundary_operator_applications = &mut *outputs.boundary_operator_applications;
    let diagnostics = &mut *outputs.diagnostics;
    if let Some(state) = current_state {
        crate::declarations::operators::validate_named_statement_operator_application(
            program,
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
        symbols,
        writable_roots,
        value_environment,
        diagnostics,
    );
    // A statement call's arguments are evaluated in the caller's environment
    // at the call site, so they owe the same decision-17 representability
    // proofs as expression-position arguments (whose analysis lives inside
    // expression_analysis::call). `validate_call_node` handles custody and
    // narrowing but reports no operand diagnostics of its own; without this
    // walk an unbounded operand here surfaced only at terminal proof
    // reconstruction. The destination resets to (None, Exact): an argument's
    // type comes from the callee's parameter, never the statement context.
    for argument in program.statement_table.expression_handles(call.arguments) {
        arithmetic_domains::validate_value_range_retaining_decisions(
            program,
            machine,
            current_state,
            *argument,
            value_environment,
            None,
            numerics::arithmetic::ArithmeticDomain::Exact,
            &format!(
                "machine `{}` state `{state_name}` call argument",
                machine.name
            ),
            diagnostics,
            arithmetic_decisions,
        );
    }
    // R5 frame seed: a resolved acyclic INTERNAL call preserves facts
    // outside its conservatively instantiated may-write set. Unknown,
    // unsummarized, and overlapping implementations remain
    // conservative. Authored `stores` clauses are retired; exactness
    // grows through inferred implementation summaries.
    if let Some(written) = direct_written.as_deref() {
        value_environment.invalidate_written_paths(written);
    } else {
        value_environment.invalidate_unknown_writes(program, current_state);
    }
    // R4 witness mint: a BOUNDARY callee's `ensures` re-seeds the
    // `&mut` out-arguments' places (the boundary model's citable
    // fact) -- `fw.get_size(&mut self.n)` with `ensures size <= 8`
    // leaves `self.n` in [type_low, 8].
    if let Some(signature) =
        crate::machine_calls::calls::boundary_trait_signature(program, machine, call)
    {
        arithmetic_domains::seed_out_param_ensures(
            program,
            machine,
            current_state,
            call,
            signature,
            value_environment,
        );
    } else {
        // Citation: a checked callee's proved element bounds enter the flow
        // at the exact argument places.
        arithmetic_domains::seed_cited_element_bounds(
            program,
            machine,
            current_state,
            call,
            direct_written.as_deref(),
            value_environment,
        );
    }
}
