mod cycles;
mod lookups;
mod state_keys;
mod targets;

use self::state_keys::StateKeyBuffer;
use omega_control_flow::{ControlFlowPlan, StateKey, TransitionExpressionRefs, TransitionFlow};
use omega_core::diagnostics::Diagnostic;

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
        Vec<omega_core::arena::HandleSpan<omega_checked_trees::expression::ExpressionHandle>>,
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
            context_entry_arguments: vec![omega_core::arena::HandleSpan::empty()],
        }
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
    ) -> omega_core::arena::HandleSpan<omega_checked_trees::expression::ExpressionHandle> {
        self.context_entry_arguments
            .get(context.0 as usize)
            .copied()
            .unwrap_or_else(omega_core::arena::HandleSpan::empty)
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

        self.machine_flow_by_symbol(state_key.machine)?;
        let state = self.state_flow_by_key(state_key)?;
        let transition_span = state.transitions;
        self.runtime_flow.states.insert(RuntimeState {
            key: state_key,
            context,
        });
        self.reached_states.push(node);
        self.active_states.push(node);

        if self.visit_state_call_edges(state_key, context, transition_span)? {
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
            let (statement_index, target, continuation, expressions) = {
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
                // A terminal target returns into the clone's entry continuation;
                // if that is the next chained call, its arguments ride along.
                let expressions = if matches!(raw_target, crate::RuntimeTransitionTarget::Terminal)
                {
                    TransitionExpressionRefs {
                        target_arguments: self.entry_arguments(context),
                        ..transition.expressions
                    }
                } else {
                    transition.expressions
                };

                (
                    transition.statement_index,
                    self.inherit_terminal(raw_target, context),
                    self.inherit_terminal(
                        self.runtime_target(state_key.machine, &transition.continuation, context),
                        context,
                    ),
                    expressions,
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
            )?;
        }

        self.active_states.pop();

        Ok(())
    }

    fn visit_state_call_edges(
        &mut self,
        state_key: StateKey,
        context: CallContext,
        transition_span: omega_core::arena::HandleSpan<TransitionFlow>,
    ) -> Result<bool, Diagnostic> {
        let mut call_edges: Vec<RuntimeStateCallEdge> = self
            .state_calls
            .iter()
            .copied()
            .filter(|edge| edge.source_key == state_key)
            .collect();
        if call_edges.is_empty() {
            return Ok(false);
        }
        // Multiple statement calls in one state run SEQUENTIALLY (in source
        // order), so they must CHAIN: call N returns into call N+1, and only the
        // last returns to the state's own continuation. Emitting them as parallel
        // dispatch edges that all return to the final continuation is wrong -- the
        // dispatch loop only ever takes the first, silently skipping the rest.
        call_edges.sort_by_key(|edge| edge.statement_index);

        // Reject genuine call recursion (a state whose calls transitively call
        // back into it) up front, so the cloning DFS cannot overflow. This is a
        // STATIC check over call + transition edges -- unlike the old dynamic
        // `active_states` check it does not follow continuations, so chaining a
        // state's sequential calls (where a call's continuation is the next
        // sibling call) is not mistaken for recursion.
        for call_edge in &call_edges {
            if self.call_target_recurses_into(call_edge.target_key, state_key) {
                return Err(Diagnostic::error(format!(
                    "{} calls into a recursive cycle (target {}); specialization \
                     cannot lower a call graph that (transitively) calls itself. \
                     Express the repetition as a loop (a self-transition) instead.",
                    self.state_key_display(state_key),
                    self.state_key_display(call_edge.target_key)
                )));
            }
        }

        // What the LAST call returns to: the state's own continuation (its
        // transition, or its clone's entry continuation if it is a tail call).
        let final_continuation = self.continuation_after_statement_call(
            state_key,
            context,
            transition_span,
            *call_edges.last().expect("call_edges is non-empty"),
        )?;

        // Build each call's specialized-clone target in REVERSE so call N's
        // continuation is call N+1's already-minted target.
        let mut targets =
            vec![crate::RuntimeTransitionTarget::None; call_edges.len()];
        for index in (0..call_edges.len()).rev() {
            let continuation = if index + 1 < call_edges.len() {
                targets[index + 1]
            } else {
                final_continuation
            };
            // When this call's clone terminates it returns into the next chained
            // call, so its entry arguments are that call's arguments.
            let return_arguments = if index + 1 < call_edges.len() {
                self.statement_call_arguments(state_key, call_edges[index + 1].statement_index)
            } else {
                omega_core::arena::HandleSpan::empty()
            };
            let callee_context = self.next_callee_context(continuation, return_arguments)?;
            targets[index] = crate::RuntimeTransitionTarget::State {
                key: call_edges[index].target_key,
                context: callee_context,
            };
        }

        // Emit each call edge with its chained continuation. Carry the call's
        // argument expressions so the callee's parameter slots are materialized
        // when entered. The dispatch loop takes the first edge; each call returns
        // (via its continuation) into the next, then the last into the state's
        // continuation.
        for index in 0..call_edges.len() {
            let continuation = if index + 1 < call_edges.len() {
                targets[index + 1]
            } else {
                final_continuation
            };
            let target_arguments =
                self.statement_call_arguments(state_key, call_edges[index].statement_index);
            // When this call returns into the NEXT chained call, that call's
            // arguments ride along as the continuation arguments so they are
            // materialized as the dispatch loop enters it.
            let continuation_arguments = if index + 1 < call_edges.len() {
                self.statement_call_arguments(state_key, call_edges[index + 1].statement_index)
            } else {
                omega_core::arena::HandleSpan::empty()
            };
            self.visit_transition(
                state_key,
                context,
                call_edges[index].statement_index,
                targets[index],
                continuation,
                TransitionExpressionRefs {
                    target_arguments,
                    continuation_arguments,
                    ..TransitionExpressionRefs::default()
                },
            )?;
        }

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
        return_arguments: omega_core::arena::HandleSpan<
            omega_checked_trees::expression::ExpressionHandle,
        >,
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
        Ok(context)
    }

    /// The argument expression handles (in the control-flow expression table) of
    /// the state call statement at `statement_index` within `state_key`.
    fn statement_call_arguments(
        &self,
        state_key: StateKey,
        statement_index: usize,
    ) -> omega_core::arena::HandleSpan<omega_checked_trees::expression::ExpressionHandle> {
        let Ok(state) = self.state_flow_by_key(state_key) else {
            return omega_core::arena::HandleSpan::empty();
        };
        self.control_flow
            .operations
            .span(state.operations)
            .into_iter()
            .flatten()
            .find(|operation| operation.statement_index == statement_index)
            .and_then(|operation| match operation.expressions {
                omega_control_flow::OperationExpressionRefs::Call { arguments } => Some(arguments),
                _ => None,
            })
            .unwrap_or_else(omega_core::arena::HandleSpan::empty)
    }

    fn continuation_after_statement_call(
        &self,
        state_key: StateKey,
        context: CallContext,
        transition_span: omega_core::arena::HandleSpan<TransitionFlow>,
        call_edge: RuntimeStateCallEdge,
    ) -> Result<crate::RuntimeTransitionTarget, Diagnostic> {
        let transitions = self
            .control_flow
            .transitions
            .span(transition_span)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "{} has an invalid transition span",
                    self.state_key_display(state_key)
                ))
            })?;
        let continuation_transition = transitions
            .iter()
            .find(|transition| transition.statement_index > call_edge.statement_index)
            .or_else(|| transitions.first());

        // The continuation lives in the caller's machine, so it inherits the
        // caller's context -- this is what wires a clone's terminal back to the
        // specific call site that entered it. When the call is a tail call (no
        // local continuation), the caller in turn returns to ITS entry
        // continuation, threading the original caller's continuation down the
        // chain.
        Ok(continuation_transition
            .map(|transition| self.runtime_target(state_key.machine, &transition.target, context))
            .map(|target| self.inherit_terminal(target, context))
            .unwrap_or_else(|| self.entry_continuation(context)))
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
