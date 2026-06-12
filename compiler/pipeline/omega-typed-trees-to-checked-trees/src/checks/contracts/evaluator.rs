use std::cell::RefCell;

use omega_checked_trees::{FlowCallFact, FlowStateFact};
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::state::State;

mod booleans;
mod collections;
mod integers;
mod resolution;

pub(super) fn call_site_proves_boolean_contract_expression(
    program: &omega_typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_site: &crate::CallSite<'_>,
    target_state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(caller_state) =
        crate::find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return false;
    };

    ContractExpressionEvaluator {
        program,
        caller_state,
        statement_index: call_flow.statement_index,
        call_site,
        target_state,
        active_evaluations: RefCell::new(Vec::new()),
        active_resolutions: RefCell::new(Vec::new()),
    }
    .boolean_value(expression)
    .unwrap_or(false)
}

pub(super) struct ContractExpressionEvaluator<'program, 'call> {
    program: &'program omega_typed_trees::TypedTrees,
    caller_state: &'program State,
    statement_index: usize,
    call_site: &'call crate::CallSite<'program>,
    target_state: &'program State,
    /// Expressions whose integer evaluation is in progress on the call
    /// stack. Following a SELF call site maps a parameter back to an
    /// argument that mentions that same parameter (`n` resolves to
    /// `n - 1`, whose `n` resolves to `n - 1` again), so the constant
    /// walk would otherwise re-enter the same expression handle forever.
    active_evaluations: RefCell<Vec<ExpressionHandle>>,
    /// Expressions whose name resolution is in progress on the call
    /// stack; catches resolution-only cycles (an argument swap such as
    /// `self.step(b, a)` makes parameter `a` resolve to `b` and back)
    /// that never pass through integer evaluation.
    active_resolutions: RefCell<Vec<ExpressionHandle>>,
}

impl ContractExpressionEvaluator<'_, '_> {
    /// Runs `evaluate` with `expression` marked active in `active`. If the
    /// expression is already active, the walk has looped back into an
    /// expression it is still working on -- a cycle through call-site
    /// argument following -- so stand down with None. Unknown never proves
    /// a contract and never falsely rejects one: the caller simply falls
    /// back to the other provers (arm facts, caller requires).
    pub(super) fn guarding_cycles<T>(
        active: &RefCell<Vec<ExpressionHandle>>,
        expression: ExpressionHandle,
        evaluate: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        if active.borrow().contains(&expression) {
            return None;
        }
        active.borrow_mut().push(expression);
        let value = evaluate();
        active.borrow_mut().pop();
        value
    }
}
