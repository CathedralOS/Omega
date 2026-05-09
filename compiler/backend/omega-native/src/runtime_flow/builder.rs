mod cycles;
mod lookups;
mod targets;

use super::model::{RuntimeEdge, RuntimeFlowPlan, RuntimeState, RuntimeTransitionTarget};
use omega_control_flow::{ControlFlowPlan, MachineFlow, TransitionFlow};
use omega_core::diagnostics::Diagnostic;

pub(super) struct RuntimeFlowBuilder<'plan> {
    control_flow: &'plan ControlFlowPlan,
    runtime_flow: RuntimeFlowPlan,
    active_states: Vec<RuntimeState>,
    reached_states: Vec<RuntimeState>,
}

impl<'plan> RuntimeFlowBuilder<'plan> {
    pub(super) fn new(control_flow: &'plan ControlFlowPlan) -> Self {
        Self {
            control_flow,
            runtime_flow: RuntimeFlowPlan::default(),
            active_states: Vec::new(),
            reached_states: Vec::new(),
        }
    }

    pub(super) fn finish(self) -> RuntimeFlowPlan {
        self.runtime_flow
    }

    pub(super) fn visit_state(&mut self, state_key: RuntimeState) -> Result<(), Diagnostic> {
        if self
            .active_states
            .iter()
            .any(|active_state| active_state == &state_key)
        {
            self.record_cycle_to(&state_key);
            return Ok(());
        }

        if self
            .reached_states
            .iter()
            .any(|reached_state| reached_state == &state_key)
        {
            return Ok(());
        }

        let machine = self.machine_flow_by_symbol(state_key.key.machine)?.clone();
        let state = self.state_flow_by_key(state_key.key)?;
        let transition_span = state.transitions;
        self.runtime_flow.states.insert(state_key.clone());
        self.reached_states.push(state_key.clone());
        self.active_states.push(state_key.clone());

        let transitions = self
            .control_flow
            .transitions
            .span(transition_span)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "{} has an invalid transition span",
                    self.state_key_display(state_key.key)
                ))
            })?
            .to_vec();

        for transition in transitions {
            self.visit_transition(&machine, &state_key, &transition)?;
        }

        self.active_states.pop();

        Ok(())
    }

    fn visit_transition(
        &mut self,
        machine: &MachineFlow,
        from: &RuntimeState,
        transition: &TransitionFlow,
    ) -> Result<(), Diagnostic> {
        let target = self.runtime_target(machine, &transition.target);
        let continuation = transition
            .continuation
            .as_ref()
            .map(|target| self.runtime_target(machine, target))
            .unwrap_or(RuntimeTransitionTarget::None);
        let forms_cycle = self.target_is_active(&target);

        self.runtime_flow.edges.insert(RuntimeEdge {
            from: from.key,
            target: target.clone(),
            continuation: continuation.clone(),
            guard: transition.guard.clone(),
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
