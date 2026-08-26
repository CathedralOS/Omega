mod cycles;
mod lookups;
mod state_keys;
mod targets;

use self::state_keys::StateKeyBuffer;
use omega_control_flow::{ControlFlowPlan, StateKey, TransitionExpressionRefs};
use psi_diagnostics::Diagnostic;

use crate::{CallContext, RuntimeEdge, RuntimeFlowPlan, RuntimeState, RuntimeStateCallEdge};

pub(super) struct RuntimeFlowBuilder<'plan> {
    control_flow: &'plan ControlFlowPlan,
    state_calls: &'plan [RuntimeStateCallEdge],
    runtime_flow: RuntimeFlowPlan,
    active_states: StateKeyBuffer,
    reached_states: StateKeyBuffer,
    /// Next call-context id to hand out (`ROOT`/0 is the entry machine). Each
    /// dispatched call site is specialized under a fresh context.
    next_context: u32,
    /// Upper bound on minted contexts. An acyclic call graph specializes a
    /// bounded number of copies; exceeding this means the call graph is
    /// (transitively) recursive, which specialization cannot lower.
    context_budget: u32,
    /// The continuation a clone returns to when it terminates, indexed by context
    /// id. `ROOT` returns `Terminal` (the program ends); a callee returns to the
    /// continuation its call site specified. This is what lets a tail-call chain
    /// (a clone whose body ends in another call) thread the original caller's
    /// continuation all the way down.
    context_entry_continuation: Vec<crate::RuntimeTransitionTarget>,
    /// The arguments to materialize when a clone returns into its entry
    /// continuation, indexed by context id. For a chained sequential call this is
    /// the NEXT call's arguments (machine-owned, so frame-independent); for a true
    /// tail call it is empty.
    context_entry_arguments:
        Vec<psi_arena::HandleSpan<psi_checked_trees::expression::ExpressionHandle>>,
    /// The caller call-result slot a clone returns into, indexed by context id.
    /// `Some` only for a clone created by a VALUE-position call (`let n = f(..)`):
    /// its terminal value is written to this slot when the clone returns. `None`
    /// for ROOT and statement-position calls.
    context_call_result: Vec<Option<crate::CallResultReturn>>,
}

impl<'plan> RuntimeFlowBuilder<'plan> {
    pub(super) fn new(
        control_flow: &'plan ControlFlowPlan,
        state_calls: &'plan [RuntimeStateCallEdge],
    ) -> Self {
        let state_capacity = control_flow.states.len();
        let edge_capacity = control_flow
            .transitions
            .len()
            .saturating_add(state_calls.len());

        Self {
            control_flow,
            state_calls,
            runtime_flow: RuntimeFlowPlan::with_capacity(
                state_capacity,
                edge_capacity,
                state_capacity,
                state_capacity,
            ),
            active_states: StateKeyBuffer::with_capacity(state_capacity),
            reached_states: StateKeyBuffer::with_capacity(state_capacity),
            next_context: CallContext::ROOT.0 + 1,
            // Generous: real (acyclic) programs need at most one context per
            // call-path. The bound only fires on genuine call recursion or a
            // pathological specialization blow-up.
            context_budget: (state_capacity as u32)
                .saturating_mul(64)
                .saturating_add(1024),
            // Index 0 == ROOT: the entry machine terminates the program.
            context_entry_continuation: vec![crate::RuntimeTransitionTarget::Terminal],
            context_entry_arguments: vec![psi_arena::HandleSpan::empty()],
            context_call_result: vec![None],
        }
    }

    /// Record the minting call site for `context` (parallel to the plan's
    /// `context_call_sites`; ROOT stays the invalid placeholder). The third
    /// element is the call ordinal and the fourth is the PARENT context -- the
    /// one the calling state ran in.
    fn record_context_call_site(
        &mut self,
        context: CallContext,
        site: (StateKey, usize, usize, CallContext),
    ) {
        let index = context.0 as usize;
        debug_assert_eq!(self.runtime_flow.context_call_sites.len(), index);
        let _ = index;
        self.runtime_flow.context_call_sites.push(site);
    }

    /// The continuation a clone in `context` returns to on termination.
    fn entry_continuation(&self, context: CallContext) -> crate::RuntimeTransitionTarget {
        self.context_entry_continuation
            .get(context.0 as usize)
            .copied()
            .unwrap_or(crate::RuntimeTransitionTarget::Terminal)
    }

    /// The arguments to materialize when a clone in `context` returns into its
    /// entry continuation (the next chained call's arguments, or empty).
    fn entry_arguments(
        &self,
        context: CallContext,
    ) -> psi_arena::HandleSpan<psi_checked_trees::expression::ExpressionHandle> {
        self.context_entry_arguments
            .get(context.0 as usize)
            .copied()
            .unwrap_or_else(psi_arena::HandleSpan::empty)
    }

    /// The caller call-result slot a clone in `context` returns into (`Some` only
    /// for a value-position call's clone), or `None`.
    fn context_call_result(&self, context: CallContext) -> Option<crate::CallResultReturn> {
        self.context_call_result
            .get(context.0 as usize)
            .copied()
            .flatten()
    }

    pub(super) fn finish(self) -> RuntimeFlowPlan {
        self.runtime_flow
    }

    pub(super) fn visit_state(
        &mut self,
        state_key: StateKey,
        context: CallContext,
    ) -> Result<(), Diagnostic> {
        let node = (state_key, context);
        if self.active_states.contains(&node) {
            self.record_cycle_to(state_key, context);
            return Ok(());
        }

        if self.reached_states.contains(&node) {
            return Ok(());
        }

        // A state that interleaves dispatched calls with inline operations is
        // lowered as a chain of SEGMENT sub-states (same machine+state+context,
        // increasing `segment_index`). A dispatch case runs all its inline ops
        // before dispatching, so each dispatched call must end a segment: segment
        // `i` runs the ops up to and including call `i`, dispatches it, and returns
        // into segment `i+1`; the final segment runs the trailing ops and the
        // state's real transition. Segments share a context, so they share the
        // state's frame (params land at identical offsets) -- the call's arguments
        // and the trailing ops both resolve against the original state's locals.
        let control_key = StateKey {
            segment_index: 0,
            ..state_key
        };
        self.machine_flow_by_symbol(control_key.machine)?;
        let state = self.state_flow_by_key(control_key)?;
        let transition_span = state.transitions;
        self.runtime_flow.states.insert(RuntimeState {
            key: state_key,
            context,
        });
        self.reached_states.push(node);
        self.active_states.push(node);

        if self.visit_state_segment_call(state_key, control_key, context)? {
            self.active_states.pop();

            return Ok(());
        }

        let transition_count = self
            .control_flow
            .transitions
            .span(transition_span)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "{} has an invalid transition span",
                    self.state_key_display(state_key)
                ))
            })?
            .len();

        // A state with no transitions is terminal; otherwise track whether any
        // transition is unconditional (no guard) so we know if every runtime path
        // is covered.
        let mut has_unconditional = transition_count == 0;
        for transition_index in 0..transition_count {
            let (statement_index, target, continuation, expressions, call_result) = {
                let transition = self
                    .control_flow
                    .transitions
                    .span(transition_span)
                    .and_then(|transitions| transitions.get(transition_index))
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "{} has an invalid transition span",
                            self.state_key_display(state_key)
                        ))
                    })?;

                let raw_target =
                    self.runtime_target(state_key.machine, &transition.target, context);
                let is_terminal = matches!(raw_target, crate::RuntimeTransitionTarget::Terminal);
                // A terminal target returns into the clone's entry continuation;
                // if that is the next chained call, its arguments ride along.
                let expressions = if is_terminal {
                    TransitionExpressionRefs {
                        target_arguments: self.entry_arguments(context),
                        ..transition.expressions
                    }
                } else {
                    transition.expressions
                };
                // A value-returning callee's terminal (`-> acc` / `-> 99`) carries
                // the caller call-result slot so selection writes the value back.
                let call_result = if is_terminal && transition.expressions.target_value.is_valid() {
                    self.context_call_result(context)
                } else {
                    None
                };

                (
                    transition.statement_index,
                    self.inherit_terminal(raw_target, context),
                    self.inherit_terminal(
                        self.runtime_target(state_key.machine, &transition.continuation, context),
                        context,
                    ),
                    expressions,
                    call_result,
                )
            };

            if !expressions.guard.is_valid() {
                has_unconditional = true;
            }

            self.visit_transition(
                state_key,
                context,
                statement_index,
                target,
                continuation,
                expressions,
                call_result,
            )?;
        }

        // If every transition is guarded, no edge is taken when none of the
        // guards hold -- in the dispatch loop that re-enters this same state
        // forever. Add an unconditional fall-through (tried last, after the
        // guarded edges) to the clone's return point, matching the inline path's
        // "no transition matched -> the machine returns" behavior.
        if !has_unconditional {
            let fall_through = self.entry_continuation(context);
            let fall_through_arguments = self.entry_arguments(context);
            self.visit_transition(
                state_key,
                context,
                0,
                fall_through,
                crate::RuntimeTransitionTarget::None,
                TransitionExpressionRefs {
                    target_arguments: fall_through_arguments,
                    ..TransitionExpressionRefs::default()
                },
                None,
            )?;
        }

        self.active_states.pop();

        Ok(())
    }

    /// If this segment (`state_key.segment_index`) of `control_key` dispatches a
    /// call, emit that one call edge (returning into the next segment) and return
    /// `true`. Returns `false` when there is no call for this segment (the state
    /// has no dispatched calls, or this is the tail segment past the last call),
    /// so the caller runs the state's transitions instead.
    fn visit_state_segment_call(
        &mut self,
        state_key: StateKey,
        control_key: StateKey,
        context: CallContext,
    ) -> Result<bool, Diagnostic> {
        let mut call_edges: Vec<RuntimeStateCallEdge> = self
            .state_calls
            .iter()
            .copied()
            .filter(|edge| edge.source_key == control_key)
            .collect();
        if call_edges.is_empty() {
            return Ok(false);
        }
        call_edges.sort_by_key(|edge| (edge.statement_index, edge.call_ordinal));

        let segment = state_key.segment_index;
        if segment >= call_edges.len() {
            // Tail segment: past the last call. Run the trailing inline ops and
            // the state's real transition (handled by the caller).
            return Ok(false);
        }
        let call_edge = call_edges[segment];

        // Reject genuine call recursion (a state whose calls transitively call
        // back into it) up front, so the cloning DFS cannot overflow. STATIC check
        // over call + transition edges; it does not follow continuations.
        // OWNER DIRECTIVE (2026-07-07, TASKS.md): Omega has NO recursion --
        // stack size must be predictable; repetition is a bare state
        // self-transition loop. A tail-call-to-loop transform that ACCEPTED
        // this shape landed 2026-07-09/10 (fs lane, unaware of the directive)
        // and was RETRACTED 2026-07-10k; the terminal-position spelling is
        // pinned rejected by fail/calls/terminal_self_call_recursion_rejected.
        if self.call_target_recurses_into(call_edge.target_key, control_key) {
            return Err(Diagnostic::error(format!(
                "{} calls into a recursive cycle (target {}); specialization \
                 cannot lower a call graph that (transitively) calls itself. \
                 Express the repetition as a loop (a self-transition) instead.",
                self.state_key_display(control_key),
                self.state_key_display(call_edge.target_key)
            )));
        }

        // The call returns into the NEXT segment of the SAME state. Segments share
        // a context (hence the state's frame), so nothing is re-materialized on
        // return -- the trailing ops read the state's own locals directly.
        let next_segment_target = crate::RuntimeTransitionTarget::State {
            key: StateKey {
                segment_index: segment + 1,
                ..control_key
            },
            context,
        };
        // A value-position call (`let n = count(..)`) carries a call-result slot;
        // its callee clone writes the terminal value back there when it returns.
        // A statement call (`count(..)`) discards the result -> no slot.
        let call_result = if call_edge.is_value {
            Some(crate::CallResultReturn {
                call_source_key: control_key,
                statement_index: call_edge.statement_index,
                call_ordinal: call_edge.call_ordinal,
            })
        } else {
            None
        };
        let callee_context = self.next_callee_context(
            next_segment_target,
            psi_arena::HandleSpan::empty(),
            call_result,
        )?;
        self.record_context_call_site(
            callee_context,
            (
                control_key,
                call_edge.statement_index,
                call_edge.call_ordinal,
                context,
            ),
        );
        let call_target = crate::RuntimeTransitionTarget::State {
            key: call_edge.target_key,
            context: callee_context,
        };
        // The call's arguments resolve against THIS segment's frame, which is the
        // state's frame (shared across segments), so params declared in the state
        // are visible here even though earlier segments dispatched first.
        let target_arguments =
            self.statement_call_arguments(control_key, call_edge.statement_index);
        self.visit_transition(
            state_key,
            context,
            call_edge.statement_index,
            call_target,
            next_segment_target,
            TransitionExpressionRefs {
                target_arguments,
                ..TransitionExpressionRefs::default()
            },
            None,
        )?;

        Ok(true)
    }

    /// Mint a fresh call-context for a dispatched callee so it is specialized as
    /// its own clone (distinct dispatch cases + frame slots), recording the
    /// continuation it returns to when it terminates. Errors if the context
    /// budget is exhausted, which indicates a recursive call graph specialization
    /// cannot lower.

    /// Whether the callee `target` can transitively reach the calling state
    /// `origin` again by following CALL and intra-machine TRANSITION edges. If so
    /// the call graph is recursive (`origin` calls `target` ... calls `origin`),
    /// which per-call-context specialization would clone forever. This deliberately
    /// does NOT follow continuation edges, so a state's chained sequential calls
    /// (call N's continuation is call N+1) are not mistaken for recursion.
    fn call_target_recurses_into(&self, target: StateKey, origin: StateKey) -> bool {
        let mut stack = vec![target];
        let mut visited: Vec<StateKey> = Vec::new();
        while let Some(state) = stack.pop() {
            if state == origin {
                return true;
            }
            if visited.contains(&state) {
                continue;
            }
            visited.push(state);

            if let Some(flow) = self
                .control_flow
                .states
                .iter()
                .map(|(_, flow)| flow)
                .find(|flow| flow.key == state)
            {
                for transition in self
                    .control_flow
                    .transitions
                    .span(flow.transitions)
                    .into_iter()
                    .flatten()
                {
                    if let omega_control_flow::PlannedTransitionTarget::State { key, .. } =
                        transition.target
                    {
                        stack.push(key);
                    }
                }
            }

            for call in self
                .state_calls
                .iter()
                .filter(|call| call.source_key == state)
            {
                stack.push(call.target_key);
            }
        }
        false
    }

    fn next_callee_context(
        &mut self,
        return_continuation: crate::RuntimeTransitionTarget,
        return_arguments: psi_arena::HandleSpan<psi_checked_trees::expression::ExpressionHandle>,
        call_result: Option<crate::CallResultReturn>,
    ) -> Result<CallContext, Diagnostic> {
        if self.next_context >= self.context_budget {
            return Err(Diagnostic::error(
                "state-call specialization exceeded its budget: the call graph is \
                 (transitively) recursive. A machine that needs to repeat must do so \
                 with a loop (a self-transition), not by calling itself.",
            ));
        }

        let context = CallContext(self.next_context);
        self.next_context += 1;
        debug_assert_eq!(self.context_entry_continuation.len(), context.0 as usize);
        self.context_entry_continuation.push(return_continuation);
        self.context_entry_arguments.push(return_arguments);
        self.context_call_result.push(call_result);
        Ok(context)
    }

    /// The argument expression handles (in the control-flow expression table) of
    /// the state call statement at `statement_index` within `state_key`.
    fn statement_call_arguments(
        &self,
        state_key: StateKey,
        statement_index: usize,
    ) -> psi_arena::HandleSpan<psi_checked_trees::expression::ExpressionHandle> {
        let Ok(state) = self.state_flow_by_key(state_key) else {
            return psi_arena::HandleSpan::empty();
        };
        // A statement state call's arguments may live on the operation directly
        // (`f(x)` -> Call) or inside the value expression of a value-position call
        // (`let n = f(x)` -> a LocalData/Assignment whose value expression is the
        // Call). Without descending into the value expression, a dispatched value
        // call materializes NO arguments and the callee runs on garbage.
        for operation in self
            .control_flow
            .operations
            .span(state.operations)
            .into_iter()
            .flatten()
            .filter(|operation| operation.statement_index == statement_index)
        {
            let value = match operation.expressions {
                omega_control_flow::OperationExpressionRefs::Call { arguments } => {
                    return arguments;
                }
                omega_control_flow::OperationExpressionRefs::Assignment { value, .. } => value,
                omega_control_flow::OperationExpressionRefs::Expression(value) => value,
                omega_control_flow::OperationExpressionRefs::None => continue,
            };
            if let Some(arguments) = self.first_call_arguments(value) {
                return arguments;
            }
        }
        // A TRANSITION-EMBEDDED call (`true -> check(self.count(3, 0, 0))`)
        // has NO operation at its statement -- the call lives in the
        // transition's TARGET-ARGUMENT (or guard) expressions. Without this
        // descent the dispatched callee materialized NO arguments and ran on
        // ZII (the direct spelling returned 0 while the let-bound twin
        // worked; probed via backend_report diff 2026-07-09).
        for transition in self
            .control_flow
            .transitions
            .span(state.transitions)
            .into_iter()
            .flatten()
        {
            for argument_span in [
                transition.expressions.target_arguments,
                transition.expressions.continuation_arguments,
            ] {
                for offset in 0..argument_span.count() {
                    let argument = self
                        .control_flow
                        .expressions
                        .expression_handle_at_offset(argument_span, offset);
                    if let Some(arguments) = self.first_call_arguments(argument) {
                        return arguments;
                    }
                }
            }
            if transition.expressions.guard.is_valid()
                && let Some(arguments) = self.first_call_arguments(transition.expressions.guard)
            {
                return arguments;
            }
            // A TERMINAL-embedded call (`state step(..) -> i32 { self.sum(n -
            // 1, acc + n) }`): the call lives in the terminal's TARGET_VALUE.
            // Without this leg the tail-call-to-loop rewrite's transition
            // carried NO target arguments -- params never rebound and the
            // emitted loop spun forever (probed 2026-07-10h: n stayed 4).
            if transition.expressions.target_value.is_valid()
                && let Some(arguments) =
                    self.first_call_arguments(transition.expressions.target_value)
            {
                return arguments;
            }
        }
        psi_arena::HandleSpan::empty()
    }

    /// The argument span of the first call expression reachable from `expression`
    /// (descending operand subexpressions), or `None` if there is no call.
    fn first_call_arguments(
        &self,
        expression: psi_checked_trees::expression::ExpressionHandle,
    ) -> Option<psi_arena::HandleSpan<psi_checked_trees::expression::ExpressionHandle>> {
        use psi_checked_trees::expression::ExpressionNode;
        match self.control_flow.expressions.expression(expression) {
            ExpressionNode::Call(call) => Some(call.arguments),
            ExpressionNode::Binary(binary) => self
                .first_call_arguments(binary.left)
                .or_else(|| self.first_call_arguments(binary.right)),
            _ => None,
        }
    }

    /// Resolve a `Terminal` target to the clone's entry continuation, so a clone
    /// returns to its call site instead of ending the program. Non-terminal
    /// targets pass through unchanged.
    fn inherit_terminal(
        &self,
        target: crate::RuntimeTransitionTarget,
        context: CallContext,
    ) -> crate::RuntimeTransitionTarget {
        if matches!(target, crate::RuntimeTransitionTarget::Terminal) {
            self.entry_continuation(context)
        } else {
            target
        }
    }

    fn visit_transition(
        &mut self,
        from: StateKey,
        from_context: CallContext,
        statement_index: usize,
        target: crate::RuntimeTransitionTarget,
        continuation: crate::RuntimeTransitionTarget,
        expressions: TransitionExpressionRefs,
        call_result: Option<crate::CallResultReturn>,
    ) -> Result<(), Diagnostic> {
        let forms_cycle = self.target_is_active(&target);

        self.runtime_flow.edges.insert(RuntimeEdge {
            from,
            from_context,
            statement_index,
            target,
            continuation,
            expressions,
            forms_cycle,
            call_result,
        });

        if forms_cycle {
            self.record_cycle_target(&target);
            return Ok(());
        }

        self.visit_target(target)?;
        self.visit_target(continuation)
    }
}

pub fn build_runtime_flow_plan(
    control_flow: &ControlFlowPlan,
    entry_key: omega_control_flow::StateKey,
) -> Result<RuntimeFlowPlan, Diagnostic> {
    build_runtime_flow_plan_with_state_calls(control_flow, entry_key, &[])
}

pub fn build_runtime_flow_plan_with_state_calls(
    control_flow: &ControlFlowPlan,
    entry_key: omega_control_flow::StateKey,
    state_calls: &[RuntimeStateCallEdge],
) -> Result<RuntimeFlowPlan, Diagnostic> {
    let mut builder = RuntimeFlowBuilder::new(control_flow, state_calls);
    builder.visit_state(entry_key, CallContext::ROOT)?;
    Ok(builder.finish())
}
