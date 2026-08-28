mod alias_flow;
mod arguments;
mod collection;
mod lookups;
mod lowering;
mod model;
mod required;

use arguments::build_call_arguments;
use collection::collect_machine_state_calls;
use lowering::state_call_lowering;
use omega_control_flow::{ControlFlowPlan, OperationKind, StateKey};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_platform_interface::HostCallPlan;
use omega_state_graph::RuntimeFlowPlan;
use required::mark_required_state_calls;
use std::sync::Arc;

pub use alias_flow::{AliasBinding, AliasFlowPlan, build_alias_flow_plan};
pub use model::{
    StateCall, StateCallArgument, StateCallArgumentKind, StateCallDynamicConformance,
    StateCallDynamicDispatch, StateCallDynamicDispatchCandidate, StateCallDynamicReceiver,
    StateCallLowering, StateCallPlan, StateCallResolution, StateCallRole,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateCallPlanningContext {
    pub control_flow: Arc<ControlFlowPlan>,
    pub host_calls: Arc<HostCallPlan>,
    pub runtime_flow: Arc<RuntimeFlowPlan>,
}

impl StateCallPlanningContext {
    fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
        expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
    }

    pub fn runtime_state_is_reachable_by_key(&self, state_key: StateKey) -> bool {
        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| state.key == state_key)
    }

    pub fn state_statement_has_host_call_by_key(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> bool {
        self.host_calls.calls.iter().any(|(_, host_call)| {
            Self::state_key_matches_statement_source(host_call.source_key, source_key)
                && host_call.statement_index == statement_index
        })
    }

    pub fn state_mutation_is_already_lowered_by_key(
        &self,
        state_key: StateKey,
        statement_index: usize,
    ) -> bool {
        let Some(machine) = self
            .control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.symbol == state_key.machine)
            .map(|(_, machine)| machine)
        else {
            return false;
        };
        let Some(state) = self
            .control_flow
            .states
            .span(machine.states)
            .and_then(|states| states.iter().find(|state| state.key == state_key))
        else {
            return false;
        };
        let Some(operations) = self.control_flow.operations.span(state.operations) else {
            return false;
        };

        operations.iter().any(|operation| {
            operation.statement_index == statement_index
                && matches!(
                    operation.kind,
                    OperationKind::ConstantIntegerAssignment | OperationKind::StaticAssignment
                )
        })
    }

    pub fn borrow_call_by_key(
        &self,
        source_key: StateKey,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<&omega_control_flow::StateBorrowCall> {
        let state = self.control_flow.state_by_key(source_key)?;
        self.control_flow
            .semantics
            .borrow
            .calls
            .span(state.borrow.calls)?
            .iter()
            .find(|call| {
                call.statement_index == statement_index && call.call_ordinal == call_ordinal
            })
    }
}

pub fn build_state_call_plan(
    control_flow: &ControlFlowPlan,
    host_calls: &HostCallPlan,
    runtime_flow: &RuntimeFlowPlan,
) -> StateCallPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_call_plan_with_workers(
        Arc::new(StateCallPlanningContext {
            control_flow: Arc::new(control_flow.clone()),
            host_calls: Arc::new(host_calls.clone()),
            runtime_flow: Arc::new(runtime_flow.clone()),
        }),
        workers.handle(),
    )
}

pub fn build_state_call_plan_with_workers(
    context: Arc<StateCallPlanningContext>,
    workers: WorkerPoolHandle,
) -> StateCallPlan {
    let machine_count = context.control_flow.machines.len();
    let context_for_collection = Arc::clone(&context);
    let machine_calls = workers.map_ordered(machine_count, move |index| {
        let machine = context_for_collection
            .control_flow
            .machines
            .storage_slice()
            .get(index)
            .expect("state-call worker index should be in range");

        collect_machine_state_calls(&context_for_collection, machine)
    });

    let call_count = machine_calls.iter().map(Vec::len).sum();
    let mut calls = Vec::with_capacity(call_count);
    for machine_calls in machine_calls {
        calls.extend(machine_calls);
    }

    let required_states = mark_required_state_calls(&context, &mut calls);

    // States that are the SOURCE of a non-host state call (in any position --
    // statement, `let x = f()`, or `g(f())`) cannot be leaves: their body performs a
    // call that must be expanded. The leaf classifier in `lowering` only scans for
    // `OperationKind::Call` ops, which misses calls in `let` initializers / argument
    // expressions; this set restores them so such a target lowers as InlineExpansion
    // (whose nested-call recursion delivers the inner call) instead of a leaf (which
    // silently drops it).
    let mut calling_states: Vec<StateKey> = Vec::new();
    for call in &calls {
        if call.target_key.is_valid()
            && matches!(
                call.role,
                StateCallRole::Statement
                    | StateCallRole::AssignmentValue
                    | StateCallRole::CallArgument
            )
            && !context.state_statement_has_host_call_by_key(call.source_key, call.statement_index)
            && !calling_states.contains(&call.source_key)
        {
            calling_states.push(call.source_key);
        }
    }

    let argument_count = collected_call_argument_count(&context, &calls);
    let mut plan = StateCallPlan::with_capacity(calls.len(), argument_count, required_states.len());
    plan.required_states.insert_many(required_states);
    for call in calls {
        let lowering = state_call_lowering(&context, &call, &calling_states);
        let arguments = build_call_arguments(
            &context,
            &mut plan.expressions,
            &mut plan.arguments,
            call.source_key,
            call.statement_index,
            call.role,
            call.call_ordinal,
            call.target_key,
            call.required,
            call.raw_arguments,
        );

        // The full receiver member path (root -> leaf). Value-position calls
        // carry their receiver EXPRESSION and walk its chain; statement calls
        // only know the leaf name, recorded as a single-segment path.
        let mut receiver_path = psi_arena::HandleSpan::empty();
        if !call.resolved_receiver_path.is_empty() {
            for segment in &call.resolved_receiver_path {
                receiver_path.push_contiguous(plan.receiver_path_segments.insert(segment.clone()));
            }
        } else if call.raw_receiver.is_valid() {
            collection::append_receiver_path(
                &context.control_flow.expressions,
                call.raw_receiver,
                &mut plan.receiver_path_segments,
                &mut receiver_path,
            );
        } else if !call.receiver_name.as_str().is_empty() {
            receiver_path.push_contiguous(
                plan.receiver_path_segments
                    .insert(call.receiver_name.clone()),
            );
        }

        plan.calls.insert(StateCall {
            source_key: call.source_key,
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            role: call.role,
            receiver_symbol: call.receiver_symbol,
            receiver_name: call.receiver_name.clone(),
            receiver_path,
            target_key: call.target_key,
            argument_count: arguments.len(),
            arguments,
            reachable: call.reachable,
            required: call.required,
            lowering,
            resolution: call.resolution,
            dynamic_dispatch: call.dynamic_dispatch.clone(),
        });
    }

    plan
}

fn collected_call_argument_count(
    context: &StateCallPlanningContext,
    calls: &[collection::CollectedStateCall],
) -> usize {
    calls
        .iter()
        .map(|call| {
            context
                .control_flow
                .expressions
                .expression_handles(call.raw_arguments)
                .len()
        })
        .sum()
}
