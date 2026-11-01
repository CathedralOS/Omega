//! Direct evaluation of a callee's Boolean contract expression at one call
//! site. The callee's parameters are replaced by the call's argument
//! expressions, and the value is computed from literals, constant integers,
//! declared integer ranges, known collection lengths and earlier immutable
//! local initializers, without reading the flow fact contexts.
//!
//! Two entries build one `ContractExpressionEvaluator` for the call and ask
//! it for `boolean_value`:
//!
//! - `call_site_proves_boolean_contract_expression`, which
//!   `prover::call_entry_contexts_prove_boolean_contract_expression` tries
//!   when the entry contexts do not prove a `requires` expression. It passes
//!   the checked operator facts, which Boolean equality needs, and an unknown
//!   value counts as not proved.
//! - `call_site_boolean_contract_expression_value`, re-exported from
//!   `contracts` for `facts::crash_calls::route_substitution`, which refines
//!   published crash routes before operator facts exist. It returns `None`
//!   for an unknown value, distinct from `false`, because a crash route may
//!   be discarded only when its guard is proved false.
//!
//! The children extend the evaluator: `booleans` evaluates `&&`, `||`, `!`
//! and comparisons, `integers` computes constant values and bounds,
//! `collections` computes collection lengths, and `resolution` follows names
//! to call arguments, local initializers, and array and struct literal
//! members. Name and integer evaluation record the expressions in progress
//! and give up on a cycle, which a call to the machine's own states can
//! create.

use checked_trees::{FlowCallFact, FlowStateFact};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;

mod booleans;
mod collections;
mod integers;
mod resolution;

#[derive(Clone, Copy)]
enum BooleanExpressionOwner {
    Contract,
    Caller,
}

pub(super) fn call_site_proves_boolean_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_site: &validation::CallSite<'_>,
    target_symbol: symbols::SymbolHandle,
    target_parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    call_site_boolean_contract_expression_value(
        program,
        Some(operators),
        state_flow,
        call_flow,
        call_site,
        target_symbol,
        target_parameters,
        expression,
    )
    .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn call_site_boolean_contract_expression_value<'program, 'call>(
    program: &'program dyn ::validation::CheckingInput,
    operators: Option<&'program checked_trees::CheckedOperatorFacts>,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_site: &'call validation::CallSite<'program>,
    target_symbol: symbols::SymbolHandle,
    target_parameters: &'program [StateParameter],
    expression: ExpressionHandle,
) -> Option<bool> {
    let caller_state = validation::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    )?;
    let caller_machine = program.machine_by_symbol(state_flow.machine_symbol)?;

    ContractExpressionEvaluator {
        program,
        operators,
        caller_machine,
        caller_state,
        statement_index: call_flow.statement_index,
        call_site,
        target_symbol,
        target_parameters,
        active_evaluations: Vec::new(),
        active_resolutions: Vec::new(),
    }
    .boolean_value(expression)
}

pub(super) struct ContractExpressionEvaluator<'program, 'call> {
    program: &'program dyn ::validation::CheckingInput,
    // Source call checking has selected operator facts. Earlier crash-fact
    // construction does not, and cannot gain Boolean equality authority here.
    operators: Option<&'program checked_trees::CheckedOperatorFacts>,
    caller_machine: &'program Machine,
    caller_state: &'program State,
    statement_index: usize,
    call_site: &'call validation::CallSite<'program>,
    target_symbol: symbols::SymbolHandle,
    target_parameters: &'program [StateParameter],
    /// Expressions whose integer evaluation is in progress on the call
    /// stack. Following a SELF call site maps a parameter back to an
    /// argument that mentions that same parameter (`n` resolves to
    /// `n - 1`, whose `n` resolves to `n - 1` again), so the constant
    /// walk would otherwise re-enter the same expression handle forever.
    active_evaluations: Vec<ExpressionHandle>,
    /// Expressions whose name resolution is in progress on the call
    /// stack; catches resolution-only cycles (an argument swap such as
    /// `self.step(b, a)` makes parameter `a` resolve to `b` and back)
    /// that never pass through integer evaluation.
    active_resolutions: Vec<ExpressionHandle>,
}

#[derive(Clone, Copy)]
enum ExpressionWork {
    Evaluation,
    Resolution,
}

impl ContractExpressionEvaluator<'_, '_> {
    /// Runs `evaluate` with `expression` marked active in `active`. If the
    /// expression is already active, the walk has looped back into an
    /// expression it is still working on -- a cycle through call-site
    /// argument following -- so stand down with None. Unknown never proves
    /// a contract and never falsely rejects one: the caller simply falls
    /// back to the other provers (arm facts, caller requires).
    fn guarding_cycles<T>(
        &mut self,
        work: ExpressionWork,
        expression: ExpressionHandle,
        evaluate: impl FnOnce(&mut Self) -> Option<T>,
    ) -> Option<T> {
        let active = match work {
            ExpressionWork::Evaluation => &mut self.active_evaluations,
            ExpressionWork::Resolution => &mut self.active_resolutions,
        };
        if active.contains(&expression) {
            return None;
        }
        active.push(expression);
        let value = evaluate(self);
        match work {
            ExpressionWork::Evaluation => self.active_evaluations.pop(),
            ExpressionWork::Resolution => self.active_resolutions.pop(),
        };
        value
    }
}
