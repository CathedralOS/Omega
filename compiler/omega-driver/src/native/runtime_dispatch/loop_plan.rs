use crate::ir::statement::TransitionGuard;
use crate::native::plan::NativePlan;
use crate::native::runtime_flow::RuntimeTransitionTarget;
use crate::native::state_guards::StateGuardLowering;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopPlan {
    pub needed: bool,
    pub entry_dispatch_index: u32,
    pub terminal_dispatch_index: u32,
    pub current_state_slot: String,
    pub next_state_slot: String,
    pub cases: Arena<RuntimeDispatchLoopCase>,
    pub edges: Arena<RuntimeDispatchLoopEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchLoopCase {
    pub machine: String,
    pub state: String,
    pub dispatch_index: u32,
    pub label: String,
    pub operation_count: usize,
    pub edges: HandleSpan<RuntimeDispatchLoopEdge>,
}

impl Default for RuntimeDispatchLoopCase {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            dispatch_index: 0,
            label: String::new(),
            operation_count: 0,
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchLoopEdge {
    pub order: usize,
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub guard: TransitionGuard,
    pub guard_lowering: StateGuardLowering,
    pub action: RuntimeDispatchLoopAction,
    pub forms_cycle: bool,
}

impl Default for RuntimeDispatchLoopEdge {
    fn default() -> Self {
        Self {
            order: 0,
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            guard: TransitionGuard::Always,
            guard_lowering: StateGuardLowering::NoOp,
            action: RuntimeDispatchLoopAction::Unknown,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeDispatchLoopAction {
    EnterState,
    Terminate,
    #[default]
    Unknown,
}

pub fn build_runtime_dispatch_loop_plan(native_plan: &NativePlan) -> RuntimeDispatchLoopPlan {
    let mut plan = RuntimeDispatchLoopPlan {
        needed: !native_plan.runtime_flow.cycles.is_empty(),
        entry_dispatch_index: dispatch_index_for_state(
            native_plan,
            &native_plan.entry_machine,
            &native_plan.entry_state,
        ),
        terminal_dispatch_index: 0,
        current_state_slot: "omega_current_state".to_owned(),
        next_state_slot: "omega_next_state".to_owned(),
        cases: Arena::new(),
        edges: Arena::new(),
    };

    if !plan.needed {
        return plan;
    }

    for (_, state) in native_plan.state_dispatch.states.iter() {
        let Some(dispatch_edges) = native_plan.state_dispatch.edges.span(state.edges) else {
            continue;
        };
        let edges = dispatch_edges
            .iter()
            .enumerate()
            .map(|(order, edge)| RuntimeDispatchLoopEdge {
                order,
                target: edge.target.clone(),
                target_dispatch_index: edge.target_dispatch_index,
                continuation: edge.continuation.clone(),
                continuation_dispatch_index: edge.continuation_dispatch_index,
                guard: edge.guard.clone(),
                guard_lowering: guard_lowering(native_plan, state.dispatch_index, order),
                action: dispatch_action(&edge.target),
                forms_cycle: edge.forms_cycle,
            })
            .collect::<Vec<_>>();
        let operation_count = runtime_body_operation_count(native_plan, state.dispatch_index);
        let edges = plan.edges.insert_many(edges);

        plan.cases.insert(RuntimeDispatchLoopCase {
            machine: state.machine.clone(),
            state: state.state.clone(),
            dispatch_index: state.dispatch_index,
            label: state.label.clone(),
            operation_count,
            edges,
        });
    }

    plan
}

fn dispatch_index_for_state(native_plan: &NativePlan, machine: &str, state: &str) -> u32 {
    native_plan
        .state_dispatch
        .states
        .iter()
        .find(|(_, dispatch_state)| {
            dispatch_state.machine == machine && dispatch_state.state == state
        })
        .map(|(_, dispatch_state)| dispatch_state.dispatch_index)
        .unwrap_or(0)
}

fn guard_lowering(
    native_plan: &NativePlan,
    source_dispatch_index: u32,
    statement_order: usize,
) -> StateGuardLowering {
    native_plan
        .state_guards
        .guards
        .iter()
        .find(|(_, guard)| {
            guard.source_dispatch_index == source_dispatch_index
                && guard.statement_order == statement_order
        })
        .map(|(_, guard)| guard.lowering)
        .unwrap_or(StateGuardLowering::NeedsRuntimeExpression)
}

fn dispatch_action(target: &RuntimeTransitionTarget) -> RuntimeDispatchLoopAction {
    match target {
        RuntimeTransitionTarget::State { .. } => RuntimeDispatchLoopAction::EnterState,
        RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {
            RuntimeDispatchLoopAction::Terminate
        }
        RuntimeTransitionTarget::Unknown { .. } => RuntimeDispatchLoopAction::Unknown,
    }
}

fn runtime_body_operation_count(native_plan: &NativePlan, dispatch_index: u32) -> usize {
    native_plan
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| body.dispatch_index == dispatch_index)
        .and_then(|(_, body)| native_plan.runtime_bodies.operations.span(body.operations))
        .map(<[_]>::len)
        .unwrap_or(0)
}
