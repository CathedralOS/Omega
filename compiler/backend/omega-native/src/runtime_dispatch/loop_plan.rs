use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyPlan;
use crate::runtime_dispatch::states::{DispatchEdge, StateDispatchPlan};
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_guards::{
    StateGuardLowering, StateGuardOperandKind, StateGuardOperandStorage, StateGuardOperator,
    StateGuardPlan,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::statement::TransitionGuard;
use std::sync::Arc;

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
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_dispatch_loop_plan_with_workers(
        Arc::new(RuntimeDispatchLoopContext::from_native_plan(native_plan)),
        runtime_dispatch_loop_inputs(native_plan),
        workers.handle(),
    )
}

pub fn build_runtime_dispatch_loop_plan_with_workers(
    context: Arc<RuntimeDispatchLoopContext>,
    case_inputs: Vec<RuntimeDispatchLoopCaseInput>,
    workers: WorkerPoolHandle,
) -> RuntimeDispatchLoopPlan {
    let mut plan = RuntimeDispatchLoopPlan {
        needed: context.needed,
        entry_dispatch_index: context.entry_dispatch_index,
        terminal_dispatch_index: 0,
        current_state_slot: "omega_current_state".to_owned(),
        next_state_slot: "omega_next_state".to_owned(),
        cases: Arena::new(),
        edges: Arena::new(),
    };

    if !plan.needed {
        return plan;
    }

    if case_inputs.is_empty() {
        return plan;
    }

    let case_inputs = Arc::new(case_inputs);
    let case_count = case_inputs.len();
    let context_for_cases = Arc::clone(&context);
    let cases = workers.map_ordered(case_count, move |index| {
        let case_input = case_inputs
            .get(index)
            .expect("runtime-dispatch-loop worker index should be in range");

        build_runtime_dispatch_loop_case(&context_for_cases, case_input)
    });

    for case in cases {
        let edges = plan.edges.insert_many(case.edges);

        plan.cases.insert(RuntimeDispatchLoopCase {
            machine: case.machine,
            state: case.state,
            dispatch_index: case.dispatch_index,
            label: case.label,
            operation_count: case.operation_count,
            edges,
        });
    }

    plan
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopContext {
    needed: bool,
    entry_dispatch_index: u32,
    state_guards: StateGuardPlan,
    runtime_bodies: RuntimeDispatchBodyPlan,
}

impl RuntimeDispatchLoopContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            needed: !native_plan.runtime_flow.cycles.is_empty(),
            entry_dispatch_index: dispatch_index_for_state(
                &native_plan.state_dispatch,
                &native_plan.entry_machine,
                &native_plan.entry_state,
            ),
            state_guards: native_plan.state_guards.clone(),
            runtime_bodies: native_plan.runtime_bodies.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopCaseInput {
    machine: String,
    state: String,
    dispatch_index: u32,
    label: String,
    edges: Vec<DispatchEdge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CollectedRuntimeDispatchLoopCase {
    machine: String,
    state: String,
    dispatch_index: u32,
    label: String,
    operation_count: usize,
    edges: Vec<RuntimeDispatchLoopEdge>,
}

pub fn runtime_dispatch_loop_inputs(native_plan: &NativePlan) -> Vec<RuntimeDispatchLoopCaseInput> {
    native_plan
        .state_dispatch
        .states
        .iter()
        .map(|(_, state)| RuntimeDispatchLoopCaseInput {
            machine: state.machine.clone(),
            state: state.state.clone(),
            dispatch_index: state.dispatch_index,
            label: state.label.clone(),
            edges: native_plan
                .state_dispatch
                .edges
                .span(state.edges)
                .unwrap_or(&[])
                .to_vec(),
        })
        .collect()
}

fn build_runtime_dispatch_loop_case(
    context: &RuntimeDispatchLoopContext,
    case_input: &RuntimeDispatchLoopCaseInput,
) -> CollectedRuntimeDispatchLoopCase {
    let edges = case_input
        .edges
        .iter()
        .enumerate()
        .map(|(order, edge)| {
            let guard_comparison =
                dispatch_guard_comparison(context, case_input.dispatch_index, order);
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
        .collect();

    CollectedRuntimeDispatchLoopCase {
        machine: case_input.machine.clone(),
        state: case_input.state.clone(),
        dispatch_index: case_input.dispatch_index,
        label: case_input.label.clone(),
        operation_count: runtime_body_operation_count(context, case_input.dispatch_index),
        edges,
    }
}

fn dispatch_index_for_state(state_dispatch: &StateDispatchPlan, machine: &str, state: &str) -> u32 {
    state_dispatch
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
    context: &RuntimeDispatchLoopContext,
    source_dispatch_index: u32,
    statement_order: usize,
) -> DispatchGuardComparison {
    let Some(guard) = context
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

    let Some(operands) = context.state_guards.operands.span(guard.operands) else {
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

fn runtime_body_operation_count(
    context: &RuntimeDispatchLoopContext,
    dispatch_index: u32,
) -> usize {
    context
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| body.dispatch_index == dispatch_index)
        .and_then(|(_, body)| context.runtime_bodies.operations.span(body.operations))
        .map(<[_]>::len)
        .unwrap_or(0)
}
