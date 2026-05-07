use crate::plan::NativePlan;
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_guards::{
    StateGuardLowering, StateGuardOperandKind, StateGuardOperandStorage, StateGuardOperator,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::statement::TransitionGuard;

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
    pub guard_operator: StateGuardOperator,
    pub guard_byte_offset: usize,
    pub guard_byte_size: usize,
    pub guard_expected_value: i64,
    pub guard_has_storage: bool,
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
            guard_operator: StateGuardOperator::None,
            guard_byte_offset: 0,
            guard_byte_size: 0,
            guard_expected_value: 0,
            guard_has_storage: false,
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
            .map(|(order, edge)| {
                let guard_comparison =
                    dispatch_guard_comparison(native_plan, state.dispatch_index, order);
                RuntimeDispatchLoopEdge {
                    order,
                    target: edge.target.clone(),
                    target_dispatch_index: edge.target_dispatch_index,
                    continuation: edge.continuation.clone(),
                    continuation_dispatch_index: edge.continuation_dispatch_index,
                    guard: edge.guard.clone(),
                    guard_lowering: guard_comparison.lowering,
                    guard_operator: guard_comparison.operator,
                    guard_byte_offset: guard_comparison.byte_offset,
                    guard_byte_size: guard_comparison.byte_size,
                    guard_expected_value: guard_comparison.expected_value,
                    guard_has_storage: guard_comparison.has_storage,
                    action: dispatch_action(&edge.target),
                    forms_cycle: edge.forms_cycle,
                }
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DispatchGuardComparison {
    lowering: StateGuardLowering,
    operator: StateGuardOperator,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    has_storage: bool,
}

fn dispatch_guard_comparison(
    native_plan: &NativePlan,
    source_dispatch_index: u32,
    statement_order: usize,
) -> DispatchGuardComparison {
    let Some(guard) = native_plan
        .state_guards
        .guards
        .iter()
        .find(|(_, guard)| {
            guard.source_dispatch_index == source_dispatch_index
                && guard.statement_order == statement_order
        })
        .map(|(_, guard)| guard)
    else {
        return DispatchGuardComparison {
            lowering: StateGuardLowering::NeedsRuntimeExpression,
            ..DispatchGuardComparison::default()
        };
    };

    let Some(operands) = native_plan.state_guards.operands.span(guard.operands) else {
        return DispatchGuardComparison {
            lowering: guard.lowering,
            operator: guard.operator,
            ..DispatchGuardComparison::default()
        };
    };
    let Some(place_operand) = operands.iter().find(|operand| {
        operand.kind == StateGuardOperandKind::Place
            && operand.storage == StateGuardOperandStorage::MachineOwned
    }) else {
        return DispatchGuardComparison {
            lowering: guard.lowering,
            operator: guard.operator,
            ..DispatchGuardComparison::default()
        };
    };
    let expected_value = operands
        .iter()
        .find(|operand| operand.has_resolved_value)
        .map(|operand| operand.resolved_value)
        .unwrap_or(0);

    DispatchGuardComparison {
        lowering: guard.lowering,
        operator: guard.operator,
        byte_offset: place_operand.byte_offset,
        byte_size: place_operand.byte_size,
        expected_value,
        has_storage: true,
    }
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
