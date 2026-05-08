use crate::control_flow::{ControlFlowPlan, MachineFlow, PlannedTransitionTarget, TransitionFlow};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeFlowPlan {
    pub states: Arena<RuntimeState>,
    pub edges: Arena<RuntimeEdge>,
    pub cycle_states: Arena<RuntimeState>,
    pub cycles: Arena<RuntimeCycle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeState {
    pub machine: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEdge {
    pub from_machine: String,
    pub from_state: String,
    pub target: RuntimeTransitionTarget,
    pub continuation: RuntimeTransitionTarget,
    pub guard: TransitionGuard,
    pub forms_cycle: bool,
}

impl Default for RuntimeEdge {
    fn default() -> Self {
        Self {
            from_machine: String::new(),
            from_state: String::new(),
            target: RuntimeTransitionTarget::Terminal,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCycle {
    pub states: HandleSpan<RuntimeState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTransitionTarget {
    State { machine: String, state: String },
    Terminal,
    None,
    Unknown { name: String },
}

impl Default for RuntimeTransitionTarget {
    fn default() -> Self {
        Self::None
    }
}

pub fn build_runtime_flow_plan(
    control_flow: &ControlFlowPlan,
    entry_machine: &str,
    entry_state: &str,
) -> Result<RuntimeFlowPlan, Diagnostic> {
    let mut builder = RuntimeFlowBuilder {
        control_flow,
        runtime_flow: RuntimeFlowPlan::default(),
        active_states: Vec::new(),
        reached_states: Vec::new(),
    };

    builder.visit_state(RuntimeState {
        machine: entry_machine.to_owned(),
        state: entry_state.to_owned(),
    })?;

    Ok(builder.runtime_flow)
}

struct RuntimeFlowBuilder<'plan> {
    control_flow: &'plan ControlFlowPlan,
    runtime_flow: RuntimeFlowPlan,
    active_states: Vec<RuntimeState>,
    reached_states: Vec<RuntimeState>,
}

impl RuntimeFlowBuilder<'_> {
    fn visit_state(&mut self, state_key: RuntimeState) -> Result<(), Diagnostic> {
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

        let machine = self.machine_flow(&state_key.machine)?.clone();
        let transition_span = self.state_flow(&machine, &state_key.state)?.transitions;
        self.runtime_flow.states.insert(state_key.clone());
        self.reached_states.push(state_key.clone());
        self.active_states.push(state_key.clone());

        let transitions = self
            .control_flow
            .transitions
            .span(transition_span)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "{}.{} has an invalid transition span",
                    state_key.machine, state_key.state
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
            from_machine: from.machine.clone(),
            from_state: from.state.clone(),
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

    fn visit_target(&mut self, target: RuntimeTransitionTarget) -> Result<(), Diagnostic> {
        if let RuntimeTransitionTarget::State { machine, state } = target {
            self.visit_state(RuntimeState { machine, state })?;
        }

        Ok(())
    }

    fn runtime_target(
        &self,
        machine: &MachineFlow,
        target: &PlannedTransitionTarget,
    ) -> RuntimeTransitionTarget {
        match target {
            PlannedTransitionTarget::State { name, .. } => RuntimeTransitionTarget::State {
                machine: machine.name.to_string(),
                state: name.to_string(),
            },
            PlannedTransitionTarget::Nested {
                receiver, state, ..
            } => machine
                .contains
                .iter()
                .find(|contained| contained.name == *receiver)
                .map(|contained| RuntimeTransitionTarget::State {
                    machine: contained.type_name.to_string(),
                    state: state.to_string(),
                })
                .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                    name: format!("{receiver}.{state}"),
                }),
            PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
                machine: machine.name.to_string(),
                state: self
                    .active_states
                    .last()
                    .map(|active_state| active_state.state.clone())
                    .unwrap_or_default(),
            },
            PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
        }
    }

    fn target_is_active(&self, target: &RuntimeTransitionTarget) -> bool {
        let RuntimeTransitionTarget::State { machine, state } = target else {
            return false;
        };

        self.active_states
            .iter()
            .any(|active_state| active_state.machine == *machine && active_state.state == *state)
    }

    fn record_cycle_target(&mut self, target: &RuntimeTransitionTarget) {
        if let RuntimeTransitionTarget::State { machine, state } = target {
            self.record_cycle_to(&RuntimeState {
                machine: machine.clone(),
                state: state.to_string(),
            });
        }
    }

    fn record_cycle_to(&mut self, target: &RuntimeState) {
        let start_index = self
            .active_states
            .iter()
            .position(|active_state| active_state == target)
            .unwrap_or(0);
        let cycle_states = self
            .active_states
            .iter()
            .skip(start_index)
            .cloned()
            .chain(std::iter::once(target.clone()))
            .collect::<Vec<_>>();
        let states = self.runtime_flow.cycle_states.insert_many(cycle_states);

        self.runtime_flow.cycles.insert(RuntimeCycle { states });
    }

    fn machine_flow(&self, machine_name: &str) -> Result<&MachineFlow, Diagnostic> {
        self.control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name == machine_name)
            .map(|(_, machine)| machine)
            .ok_or_else(|| Diagnostic::error(format!("unknown runtime machine `{machine_name}`")))
    }

    fn state_flow(
        &self,
        machine: &MachineFlow,
        state_name: &str,
    ) -> Result<&crate::control_flow::StateFlow, Diagnostic> {
        self.control_flow
            .states
            .span(machine.states)
            .and_then(|states| states.iter().find(|state| state.name == state_name))
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "unknown runtime state `{}.{state_name}`",
                    machine.name
                ))
            })
    }
}
