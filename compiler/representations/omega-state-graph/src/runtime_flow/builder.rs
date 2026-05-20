mod cycles;
mod lookups;
mod targets;

use omega_control_flow::{ControlFlowPlan, StateKey, TransitionExpressionRefs, TransitionFlow};
use omega_core::diagnostics::Diagnostic;

use crate::{RuntimeEdge, RuntimeFlowPlan, RuntimeState, RuntimeStateCallEdge};

pub(super) struct RuntimeFlowBuilder<'plan> {
    control_flow: &'plan ControlFlowPlan,
    state_calls: &'plan [RuntimeStateCallEdge],
    runtime_flow: RuntimeFlowPlan,
    active_states: StateKeyBuffer,
    reached_states: StateKeyBuffer,
}

const INLINE_RUNTIME_STATE_COUNT: usize = 32;

pub(super) struct StateKeyBuffer {
    inline: [Option<StateKey>; INLINE_RUNTIME_STATE_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl StateKeyBuffer {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_RUNTIME_STATE_COUNT],
            len: 0,
            overflow: Vec::with_capacity(state_capacity.saturating_sub(INLINE_RUNTIME_STATE_COUNT)),
        }
    }

    pub(super) fn contains(&self, key: &StateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATE_COUNT))
            .flatten()
            .any(|candidate| candidate == key)
            || self.overflow.contains(key)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &StateKey> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    fn push(&mut self, key: StateKey) {
        if self.len < INLINE_RUNTIME_STATE_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_RUNTIME_STATE_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }

    pub(super) fn last(&self) -> Option<StateKey> {
        if self.len == 0 {
            return None;
        }

        if self.len <= INLINE_RUNTIME_STATE_COUNT {
            return self.inline[self.len - 1];
        }

        self.overflow.last().copied()
    }
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
        }
    }

    pub(super) fn finish(self) -> RuntimeFlowPlan {
        self.runtime_flow
    }

    pub(super) fn visit_state(&mut self, state_key: StateKey) -> Result<(), Diagnostic> {
        if self.active_states.contains(&state_key) {
            self.record_cycle_to(state_key);
            return Ok(());
        }

        if self.reached_states.contains(&state_key) {
            return Ok(());
        }

        self.machine_flow_by_symbol(state_key.machine)?;
        let state = self.state_flow_by_key(state_key)?;
        let transition_span = state.transitions;
        self.runtime_flow
            .states
            .insert(RuntimeState { key: state_key });
        self.reached_states.push(state_key);
        self.active_states.push(state_key);

        if self.visit_state_call_edges(state_key, transition_span)? {
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

                (
                    transition.statement_index,
                    self.runtime_target(state_key.machine, &transition.target),
                    self.runtime_target(state_key.machine, &transition.continuation),
                    transition.expressions,
                )
            };

            self.visit_transition(
                state_key,
                statement_index,
                target,
                continuation,
                expressions,
            )?;
        }

        self.active_states.pop();

        Ok(())
    }

    fn visit_state_call_edges(
        &mut self,
        state_key: StateKey,
        transition_span: omega_core::arena::HandleSpan<TransitionFlow>,
    ) -> Result<bool, Diagnostic> {
        let call_edges: Vec<RuntimeStateCallEdge> = self
            .state_calls
            .iter()
            .copied()
            .filter(|edge| edge.source_key == state_key)
            .collect();
        if call_edges.is_empty() {
            return Ok(false);
        }

        for call_edge in call_edges {
            let continuation =
                self.continuation_after_statement_call(state_key, transition_span, call_edge)?;

            self.visit_transition(
                state_key,
                call_edge.statement_index,
                crate::RuntimeTransitionTarget::State {
                    key: call_edge.target_key,
                },
                continuation,
                TransitionExpressionRefs::default(),
            )?;
        }

        Ok(true)
    }

    fn continuation_after_statement_call(
        &self,
        state_key: StateKey,
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

        Ok(continuation_transition
            .map(|transition| self.runtime_target(state_key.machine, &transition.target))
            .unwrap_or(crate::RuntimeTransitionTarget::Terminal))
    }

    fn visit_transition(
        &mut self,
        from: StateKey,
        statement_index: usize,
        target: crate::RuntimeTransitionTarget,
        continuation: crate::RuntimeTransitionTarget,
        expressions: TransitionExpressionRefs,
    ) -> Result<(), Diagnostic> {
        let forms_cycle = self.target_is_active(&target);

        self.runtime_flow.edges.insert(RuntimeEdge {
            from,
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
    builder.visit_state(entry_key)?;
    Ok(builder.finish())
}
